#!/usr/bin/env tsx

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';
import { performance } from 'node:perf_hooks';

import { renderBenchmarkSvg, renderBenchmarkHtml, toolDisplayLabel } from './report.js';

import {
  resolveProjectPresetOrPath,
  parseTypeScriptDiagnostics,
  parseSurgeTsDiagnostics,
  compareDiagnostics,
  type NormalizedDiagnostic
} from '../oracle/compare-tsc.js';

const scriptPath = fileURLToPath(import.meta.url);
const scriptDir = path.dirname(scriptPath);
const workspaceRoot = path.resolve(scriptDir, '../..');
const packageManagerExecutable = process.env.npm_execpath ? process.execPath : 'pnpm';
const packageManagerArgsPrefix = process.env.npm_execpath ? [process.env.npm_execpath] : [];

type Tool = 'tsc' | 'tsgo' | 'tsgo-singleThreaded' | 'ts-rust';

type RunStats = {
  median: number;
  min: number;
  max: number;
  runs: number;
};

type BenchResult = {
  project: string;
  rustJobs: number;
  stats: Record<Tool, RunStats | null>;
  drift: Record<Tool, string>;
};

type ParsedArgs = {
  projects: string[];
  iterations: number;
  warmup: number;
  json: string | null;
  chart: string | null;
  html: string | null;
  fromJson: string | null;
  includeTsgo: boolean;
  generate: string | null;
  files: number;
  symbols: number;
  rustJobs: number;
};

const presets: Record<string, string[]> = {
  current: [
    'tests/compat-projects/lib-globals-basic/tsconfig.json',
    'tests/compat-projects/paths-basic/tsconfig.json',
    'tests/compat-projects/package-declarations/tsconfig.json',
    'tests/compat-projects/type-assertions-basic/tsconfig.json',
    'tests/compat-projects/optional-chaining-basic/tsconfig.json',
  ],
  modules: [
    'tests/compat-projects/package-imports/tsconfig.json',
    'tests/compat-projects/package-declarations/tsconfig.json',
    'tests/compat-projects/paths-basic/tsconfig.json',
  ],
  expressions: [
    'tests/compat-projects/lib-globals-basic/tsconfig.json',
    'tests/compat-projects/type-assertions-basic/tsconfig.json',
    'tests/compat-projects/optional-chaining-basic/tsconfig.json',
  ],
};

function main(argv = process.argv.slice(2)): void {
  const args = parseArgs(argv);

  if (args.fromJson) {
    const data = JSON.parse(readFileSync(args.fromJson, 'utf8'));
    let printed = false;
    if (args.chart) {
      mkdirSync(path.dirname(args.chart), { recursive: true });
      writeFileSync(args.chart, renderBenchmarkSvg(data));
      printed = true;
    }
    if (args.html) {
      mkdirSync(path.dirname(args.html), { recursive: true });
      writeFileSync(args.html, renderBenchmarkHtml(data));
      printed = true;
    }
    if (!printed) {
      printResults(data);
    }
    return;
  }

  if (args.generate) {
    const generatedProject = generateScaleFixture(args.generate, args.files, args.symbols);
    args.projects.push(generatedProject);
  }

  if (args.projects.length === 0) {
    console.error("No projects to benchmark. Provide --project or --preset or --generate.");
    process.exit(1);
  }

  const results: BenchResult[] = [];
  const tsgoAvailable = checkTsgo();

  for (const projectInput of args.projects) {
    const resolvedTsconfig = path.isAbsolute(projectInput)
      ? projectInput
      : resolveProjectLocally(projectInput);

    // Guard against ignoreDeprecations in committed fixtures
    if (!resolvedTsconfig.includes('.bench/generated') && !resolvedTsconfig.includes('target/bench')) {
      const content = readFileSync(resolvedTsconfig, 'utf8');
      if (content.includes('ignoreDeprecations')) {
        console.error(`Error: Committed fixture ${resolvedTsconfig} contains ignoreDeprecations.`);
        console.error(`This project is TS 7-oriented. Do not suppress TS 6 deprecation noise.`);
        process.exit(1);
      }
    }

    const projectDisplay = path.relative(workspaceRoot, resolvedTsconfig);
    let projectName = projectInput.split(/[/\\]/).pop() || projectInput;
    if (projectName === 'tsconfig.json') {
      const parts = projectInput.split(/[/\\]/);
      projectName = parts[parts.length - 2] || projectName;
    }

    const benchRes: BenchResult = {
      project: projectName,
      rustJobs: args.rustJobs,
      stats: { tsc: null, tsgo: null, 'tsgo-singleThreaded': null, 'ts-rust': null },
      drift: { tsc: 'baseline', tsgo: 'skipped', 'tsgo-singleThreaded': 'skipped', 'ts-rust': 'not compared' },
    };

    console.log(`Benchmarking ${projectDisplay}...`);

    // 1. Get TSC baseline and diagnostics
    console.log(`  Running tsc baseline...`);
    const tscOutput = runTool('tsc', resolvedTsconfig, 1, 0, args.rustJobs); // single run for diagnostics
    const tscDiagnostics = parseTypeScriptDiagnostics(`${tscOutput.stdout}${tscOutput.stderr}`, path.dirname(resolvedTsconfig));
    
    // Benchmark TSC
    benchRes.stats.tsc = runBenchmark('tsc', resolvedTsconfig, args.iterations, args.warmup, args.rustJobs);

    // 2. tsgo (if available)
    if (args.includeTsgo && tsgoAvailable) {
      console.log(`  Running tsgo baseline...`);
      const tsgoOutput = runTool('tsgo', resolvedTsconfig, 1, 0, args.rustJobs);
      const tsgoDiagnostics = parseTypeScriptDiagnostics(`${tsgoOutput.stdout}${tsgoOutput.stderr}`, path.dirname(resolvedTsconfig));
      const tsgoDrift = compareDrift(tscDiagnostics, tsgoDiagnostics, 'tsgo');
      benchRes.drift.tsgo = tsgoDrift;
      benchRes.stats.tsgo = runBenchmark('tsgo', resolvedTsconfig, args.iterations, args.warmup, args.rustJobs);

      // singleThreaded tsgo (optional)
      const tsgoStOutput = runTool('tsgo-singleThreaded', resolvedTsconfig, 1, 0, args.rustJobs);
      if (tsgoStOutput.exitCode !== null && !tsgoStOutput.stderr.includes('Unknown option')) {
        const tsgoStDiagnostics = parseTypeScriptDiagnostics(`${tsgoStOutput.stdout}${tsgoStOutput.stderr}`, path.dirname(resolvedTsconfig));
        benchRes.drift['tsgo-singleThreaded'] = compareDrift(tscDiagnostics, tsgoStDiagnostics, 'tsgo-singleThreaded');
        benchRes.stats['tsgo-singleThreaded'] = runBenchmark('tsgo-singleThreaded', resolvedTsconfig, args.iterations, args.warmup, args.rustJobs);
      } else {
         benchRes.drift['tsgo-singleThreaded'] = 'skipped';
      }
    } else if (args.includeTsgo && !tsgoAvailable) {
       console.log(`  tsgo skipped (not installed). Use pnpm add -g @typescript/native-preview to install.`);
    }

    // 3. surge-ts (internal tool key: ts-rust)
    console.log(`  Running surge-ts baseline...`);
    const rustOutput = runTool('ts-rust', resolvedTsconfig, 1, 0, args.rustJobs);
    const rustDiagnosticsOutput = rustOutput.stdout.trim() ? rustOutput.stdout : rustOutput.stderr;
    try {
      const rustDiagnostics = parseSurgeTsDiagnostics(rustDiagnosticsOutput, path.dirname(resolvedTsconfig));
      const rustCompare = compareDiagnostics('project', projectDisplay, tscDiagnostics, rustDiagnostics);
      if (rustCompare.summary.byCodeMatch && rustCompare.summary.byFileCodeMatch) {
         benchRes.drift['ts-rust'] = 'exact vs tsc';
      } else {
         benchRes.drift['ts-rust'] = 'known delta';
      }
    } catch (e) {
      benchRes.drift['ts-rust'] = 'parse failed';
    }
    
    benchRes.stats['ts-rust'] = runBenchmark('ts-rust', resolvedTsconfig, args.iterations, args.warmup, args.rustJobs);

    results.push(benchRes);
  }

  printResults(results);

  if (args.json) {
    mkdirSync(path.dirname(args.json), { recursive: true });
    writeFileSync(args.json, JSON.stringify(results, null, 2));
  }
  if (args.chart) {
    mkdirSync(path.dirname(args.chart), { recursive: true });
    writeFileSync(args.chart, renderBenchmarkSvg(results));
  }
  if (args.html) {
    mkdirSync(path.dirname(args.html), { recursive: true });
    writeFileSync(args.html, renderBenchmarkHtml(results));
  }
}

function resolveProjectLocally(input: string) {
  try {
     return resolveProjectPresetOrPath(input);
  } catch (e) {
     const p = path.resolve(workspaceRoot, input);
     if (existsSync(p)) return p;
     throw e;
  }
}

function checkTsgo(): boolean {
  try {
    const res = spawnSync(packageManagerExecutable, [...packageManagerArgsPrefix, 'exec', 'tsgo', '--version']);
    return res.status === 0;
  } catch {
    return false;
  }
}

function runTool(tool: Tool, tsconfig: string, runs: number, warmup: number, rustJobs: number): { exitCode: number | null, stdout: string, stderr: string, times: number[] } {
  const times: number[] = [];
  let lastOutput = { exitCode: 0 as number | null, stdout: '', stderr: '' };
  
  for (let i = 0; i < runs + warmup; i++) {
    const start = performance.now();
    let res;
    
    if (tool === 'tsc') {
      res = spawnSync(packageManagerExecutable, [...packageManagerArgsPrefix, 'exec', 'tsc', '--noEmit', '--pretty', 'false', '--project', tsconfig], { cwd: workspaceRoot, encoding: 'utf8' });
    } else if (tool === 'tsgo') {
      res = spawnSync(packageManagerExecutable, [...packageManagerArgsPrefix, 'exec', 'tsgo', '--noEmit', '--pretty', 'false', '--project', tsconfig], { cwd: workspaceRoot, encoding: 'utf8' });
    } else if (tool === 'tsgo-singleThreaded') {
      res = spawnSync(packageManagerExecutable, [...packageManagerArgsPrefix, 'exec', 'tsgo', '--noEmit', '--pretty', 'false', '--singleThreaded', '--project', tsconfig], { cwd: workspaceRoot, encoding: 'utf8' });
    } else if (tool === 'ts-rust') {
      let exePath = path.join(workspaceRoot, 'target/release/surge');
      if (process.platform === 'win32') exePath += '.exe';
      if (!existsSync(exePath)) {
        console.error(`Missing release binary: target/release/surge${process.platform === 'win32' ? '.exe' : ''}`);
        console.error(`Run: cargo build --release -p surge-ts-cli`);
        process.exit(1);
      }
      res = spawnSync(exePath, ['--project', tsconfig, '--format', 'json', '--maxDiagnostics', '10000', '--jobs', String(rustJobs)], { cwd: workspaceRoot, encoding: 'utf8' });
    } else {
      throw new Error(`Unknown tool ${tool}`);
    }

    const end = performance.now();
    
    if (i >= warmup) {
      times.push(end - start);
    }
    lastOutput = { exitCode: res.status, stdout: res.stdout || '', stderr: res.stderr || '' };
  }
  
  return { ...lastOutput, times };
}

function runBenchmark(tool: Tool, tsconfig: string, iterations: number, warmup: number, rustJobs: number): RunStats {
  const { times } = runTool(tool, tsconfig, iterations, warmup, rustJobs);
  times.sort((a, b) => a - b);
  const median = times[Math.floor(times.length / 2)] / 1000;
  const min = times[0] / 1000;
  const max = times[times.length - 1] / 1000;
  return { median, min, max, runs: iterations };
}

function compareDrift(base: NormalizedDiagnostic[], curr: NormalizedDiagnostic[], tool: string): string {
  const baseCodes = base.map(d => d.code).sort().join(',');
  const currCodes = curr.map(d => d.code).sort().join(',');
  if (baseCodes === currCodes) {
    if (base.length === curr.length) return 'exact vs tsc';
    return 'known delta'; // simplified
  }
  return 'known delta';
}

function generateScaleFixture(name: string, numFiles: number, numSymbols: number): string {
  const dir = path.join(workspaceRoot, '.bench/generated', name);
  mkdirSync(dir, { recursive: true });

  const tsconfig = {
    compilerOptions: {
      target: "es2022",
      module: "nodenext",
      moduleResolution: "nodenext",
      strict: true,
      skipLibCheck: true
    }
  };
  writeFileSync(path.join(dir, 'tsconfig.json'), JSON.stringify(tsconfig, null, 2));

  for (let i = 0; i < numFiles; i++) {
    let content = '';
    if (i > 0) {
      content += `import { Sym${i - 1}_0 } from './file${i - 1}.js';\n`;
      content += `export const imported = Sym${i - 1}_0;\n`;
    }
    
    for (let j = 0; j < numSymbols; j++) {
      content += `export interface Sym${i}_${j} { propA: string; propB?: number; }\n`;
      content += `export const val${i}_${j}: Sym${i}_${j} = { propA: "test" };\n`;
      content += `export type Alias${i}_${j} = Sym${i}_${j}[];\n`;
    }
    writeFileSync(path.join(dir, `file${i}.ts`), content);
  }

  return path.join(dir, 'tsconfig.json');
}

function printResults(results: BenchResult[]) {
  console.log('\nPerformance:');
  console.log(`${`project`.padEnd(30) + `tool`.padEnd(25) + `median`.padEnd(10) + `min`.padEnd(10) + `max`.padEnd(10)}runs`);
  for (const r of results) {
    for (const tool of ['tsc', 'tsgo', 'tsgo-singleThreaded', 'ts-rust'] as Tool[]) {
      if (r.stats[tool]) {
        const s = r.stats[tool]!;
        const toolLabel = tool === 'ts-rust' ? `${toolDisplayLabel(tool)} (jobs=${r.rustJobs})` : toolDisplayLabel(tool);
        console.log(`${`${r.project.padEnd(30)}${toolLabel.padEnd(25)}${s.median.toFixed(2)}s`.padEnd(65) + `${s.min.toFixed(2)}s`.padEnd(10) + `${s.max.toFixed(2)}s`.padEnd(10)}${s.runs}`);
      }
    }
  }

  console.log('\nDiagnostic drift:');
  console.log(`${`project`.padEnd(30) + `tool`.padEnd(25)}status`);
  for (const r of results) {
    for (const tool of ['tsgo', 'tsgo-singleThreaded', 'ts-rust'] as Tool[]) {
      if (r.drift[tool] !== 'skipped') {
         console.log(`${r.project.padEnd(30)}${toolDisplayLabel(tool).padEnd(25)}${r.drift[tool]}`);
      }
    }
  }
}

function parseArgs(argv: string[]): ParsedArgs {
  const parsed: ParsedArgs = {
    projects: [],
    iterations: 5,
    warmup: 1,
    json: null,
    chart: null,
    html: null,
    fromJson: null,
    includeTsgo: true,
    generate: null,
    files: 10,
    symbols: 50,
    rustJobs: 1,
  };

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--') continue;
    if (arg === '--project') {
      parsed.projects.push(argv[++i]);
    } else if (arg === '--preset') {
      const p = argv[++i];
      if (presets[p]) {
        parsed.projects.push(...presets[p]);
      } else {
        throw new Error(`Unknown preset: ${p}`);
      }
    } else if (arg === '--iterations') {
      parsed.iterations = parseInt(argv[++i], 10);
    } else if (arg === '--warmup') {
      parsed.warmup = parseInt(argv[++i], 10);
    } else if (arg === '--json') {
      parsed.json = argv[++i];
    } else if (arg === '--chart') {
      parsed.chart = argv[++i];
    } else if (arg === '--html') {
      parsed.html = argv[++i];
    } else if (arg === '--fromJson') {
      parsed.fromJson = argv[++i];
    } else if (arg === '--include-tsgo') {
      parsed.includeTsgo = true;
    } else if (arg === '--generate') {
      parsed.generate = argv[++i];
    } else if (arg === '--files') {
      parsed.files = parseInt(argv[++i], 10);
    } else if (arg === '--symbols') {
      parsed.symbols = parseInt(argv[++i], 10);
    } else if (arg === '--rustJobs') {
      parsed.rustJobs = parseInt(argv[++i], 10);
    }
  }

  if (!Number.isInteger(parsed.rustJobs) || parsed.rustJobs <= 0) {
    throw new Error('--rustJobs must be greater than 0');
  }

  return parsed;
}

main();
