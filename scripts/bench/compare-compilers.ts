#!/usr/bin/env tsx

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

import {
  renderBenchmarkSvg,
  renderBenchmarkHtml,
  normalizeBenchReport,
  speedupVsTsc,
  formatSpeedup,
  formatBytes,
  toolDisplayLabel,
  type BenchMemoryStats,
  type BenchReportDocument,
  type BenchReportMeta,
  type BenchReportResult,
} from './report.js';

import { runMeasuredCommand } from '../real-projects/measure-project.js';

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

// `tsc` is the legacy JS compiler (TypeScript 6.x, the `typescript-6` alias),
// kept as the slow baseline; `tsgo` is the native compiler (TypeScript 7.0, the
// canonical `typescript` package). Both packages expose a `tsc` bin and only one
// can own `.bin/tsc`, so each is invoked through its resolved package bin path.
const tsc6BinPath = path.join(workspaceRoot, 'node_modules', 'typescript-6', 'bin', 'tsc');
const tsc7BinPath = path.join(workspaceRoot, 'node_modules', 'typescript', 'bin', 'tsc');

type Tool = 'tsc' | 'tsgo' | 'tsgo-singleThreaded' | 'surge-ts';

type RunStats = {
  median: number;
  min: number;
  max: number;
  runs: number;
};

type RustJobs = number | 'auto';

type BenchResult = {
  project: string;
  rustJobs: RustJobs;
  stats: Record<Tool, RunStats | null>;
  memory: Record<Tool, BenchMemoryStats | null>;
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
  rustJobs: RustJobs;
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
    const doc = normalizeBenchReport(JSON.parse(readFileSync(args.fromJson, 'utf8')));
    let printed = false;
    if (args.chart) {
      mkdirSync(path.dirname(args.chart), { recursive: true });
      writeFileSync(args.chart, renderBenchmarkSvg(doc));
      printed = true;
    }
    if (args.html) {
      mkdirSync(path.dirname(args.html), { recursive: true });
      writeFileSync(args.html, renderBenchmarkHtml(doc));
      printed = true;
    }
    if (!printed) {
      printResults(doc.results);
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

    // Guard against ignoreDeprecations in committed fixtures. Local project
    // checkouts (.local-projects, external paths) are not ours to police.
    if (
      !resolvedTsconfig.includes('.bench/generated') &&
      !resolvedTsconfig.includes('target/bench') &&
      !resolvedTsconfig.includes('.local-projects') &&
      resolvedTsconfig.startsWith(workspaceRoot)
    ) {
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
      stats: { tsc: null, tsgo: null, 'tsgo-singleThreaded': null, 'surge-ts': null },
      memory: { tsc: null, tsgo: null, 'tsgo-singleThreaded': null, 'surge-ts': null },
      drift: { tsc: 'baseline', tsgo: 'skipped', 'tsgo-singleThreaded': 'skipped', 'surge-ts': 'not compared' },
    };

    console.log(`Benchmarking ${projectDisplay}...`);

    // 1. Get TSC baseline and diagnostics
    console.log(`  Running tsc baseline...`);
    const tscOutput = runTool('tsc', resolvedTsconfig, 1, 0, args.rustJobs); // single run for diagnostics
    const tscDiagnostics = parseTypeScriptDiagnostics(`${tscOutput.stdout}${tscOutput.stderr}`, path.dirname(resolvedTsconfig));
    
    // Benchmark TSC
    ({ stats: benchRes.stats.tsc, memory: benchRes.memory.tsc } =
      runBenchmark('tsc', resolvedTsconfig, args.iterations, args.warmup, args.rustJobs));

    // 2. tsgo (if available)
    if (args.includeTsgo && tsgoAvailable) {
      console.log(`  Running tsgo baseline...`);
      const tsgoOutput = runTool('tsgo', resolvedTsconfig, 1, 0, args.rustJobs);
      const tsgoDiagnostics = parseTypeScriptDiagnostics(`${tsgoOutput.stdout}${tsgoOutput.stderr}`, path.dirname(resolvedTsconfig));
      const tsgoDrift = compareDrift(tscDiagnostics, tsgoDiagnostics, 'tsgo');
      benchRes.drift.tsgo = tsgoDrift;
      ({ stats: benchRes.stats.tsgo, memory: benchRes.memory.tsgo } =
        runBenchmark('tsgo', resolvedTsconfig, args.iterations, args.warmup, args.rustJobs));

      // singleThreaded tsgo (optional)
      const tsgoStOutput = runTool('tsgo-singleThreaded', resolvedTsconfig, 1, 0, args.rustJobs);
      if (tsgoStOutput.exitCode !== null && !tsgoStOutput.stderr.includes('Unknown option')) {
        const tsgoStDiagnostics = parseTypeScriptDiagnostics(`${tsgoStOutput.stdout}${tsgoStOutput.stderr}`, path.dirname(resolvedTsconfig));
        benchRes.drift['tsgo-singleThreaded'] = compareDrift(tscDiagnostics, tsgoStDiagnostics, 'tsgo-singleThreaded');
        ({ stats: benchRes.stats['tsgo-singleThreaded'], memory: benchRes.memory['tsgo-singleThreaded'] } =
          runBenchmark('tsgo-singleThreaded', resolvedTsconfig, args.iterations, args.warmup, args.rustJobs));
      } else {
         benchRes.drift['tsgo-singleThreaded'] = 'skipped';
      }
    } else if (args.includeTsgo && !tsgoAvailable) {
       console.log(`  tsgo skipped (native TypeScript 7.0 not resolvable). Run pnpm install to restore the typescript package.`);
    }

    // 3. surge-ts
    console.log(`  Running surge-ts baseline...`);
    const rustOutput = runTool('surge-ts', resolvedTsconfig, 1, 0, args.rustJobs);
    const rustDiagnosticsOutput = rustOutput.stdout.trim() ? rustOutput.stdout : rustOutput.stderr;
    try {
      const rustDiagnostics = parseSurgeTsDiagnostics(rustDiagnosticsOutput, path.dirname(resolvedTsconfig));
      const rustCompare = compareDiagnostics('project', projectDisplay, tscDiagnostics, rustDiagnostics);
      if (rustCompare.summary.byCodeMatch && rustCompare.summary.byFileCodeMatch) {
         benchRes.drift['surge-ts'] = 'exact vs tsc';
      } else {
         benchRes.drift['surge-ts'] = 'known delta';
      }
    } catch (e) {
      benchRes.drift['surge-ts'] = 'parse failed';
    }

    ({ stats: benchRes.stats['surge-ts'], memory: benchRes.memory['surge-ts'] } =
      runBenchmark('surge-ts', resolvedTsconfig, args.iterations, args.warmup, args.rustJobs));

    results.push(benchRes);
  }

  printResults(results);

  const doc: BenchReportDocument = { meta: collectRunMeta(args), results };

  if (args.json) {
    mkdirSync(path.dirname(args.json), { recursive: true });
    writeFileSync(args.json, JSON.stringify(doc, null, 2));
  }
  if (args.chart) {
    mkdirSync(path.dirname(args.chart), { recursive: true });
    writeFileSync(args.chart, renderBenchmarkSvg(doc));
  }
  if (args.html) {
    mkdirSync(path.dirname(args.html), { recursive: true });
    writeFileSync(args.html, renderBenchmarkHtml(doc));
  }
}

function collectRunMeta(args: ParsedArgs): BenchReportMeta {
  const meta: BenchReportMeta = {
    timestamp: new Date().toISOString(),
    platform: `${os.platform()} ${os.arch()}`,
    nodeVersion: process.version,
    iterations: args.iterations,
    warmup: args.warmup,
  };
  const cpus = os.cpus();
  if (cpus.length > 0) {
    meta.cpu = cpus[0].model.trim();
    meta.cores = cpus.length;
  }
  const git = (gitArgs: string[]): string | undefined => {
    const res = spawnSync('git', gitArgs, { cwd: workspaceRoot, encoding: 'utf8' });
    const out = res.status === 0 ? res.stdout.trim() : '';
    return out.length > 0 ? out : undefined;
  };
  meta.gitCommit = git(['rev-parse', '--short', 'HEAD']);
  meta.gitBranch = git(['rev-parse', '--abbrev-ref', 'HEAD']);
  return meta;
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
    const res = spawnSync(process.execPath, [tsc7BinPath, '--version']);
    return res.status === 0;
  } catch {
    return false;
  }
}

function toolInvocation(tool: Tool, tsconfig: string, rustJobs: RustJobs): { command: string; args: string[] } {
  if (tool === 'tsc') {
    return { command: process.execPath, args: [tsc6BinPath, '--noEmit', '--pretty', 'false', '--project', tsconfig] };
  }
  if (tool === 'tsgo') {
    return { command: process.execPath, args: [tsc7BinPath, '--noEmit', '--pretty', 'false', '--project', tsconfig] };
  }
  if (tool === 'tsgo-singleThreaded') {
    return { command: process.execPath, args: [tsc7BinPath, '--noEmit', '--pretty', 'false', '--singleThreaded', '--project', tsconfig] };
  }
  if (tool === 'surge-ts') {
    let exePath = path.join(workspaceRoot, 'target/release/surge');
    if (process.platform === 'win32') exePath += '.exe';
    if (!existsSync(exePath)) {
      console.error(`Missing release binary: target/release/surge${process.platform === 'win32' ? '.exe' : ''}`);
      console.error(`Run: cargo build --release -p surge-ts-cli`);
      process.exit(1);
    }
    return { command: exePath, args: ['--project', tsconfig, '--format', 'json', '--maxDiagnostics', '10000', '--jobs', String(rustJobs)] };
  }
  throw new Error(`Unknown tool ${tool}`);
}

type ToolRunOutput = {
  exitCode: number | null;
  stdout: string;
  stderr: string;
  times: number[];
  footprint: Array<number | null>;
  rss: Array<number | null>;
  rssSource: string;
};

/// Each run goes through the same `/usr/bin/time` wrapper as measure-project,
/// so wall time and peak memory come from the same invocation. When peak
/// memory is unmeasurable the run still executes and the sample is null.
function runTool(tool: Tool, tsconfig: string, runs: number, warmup: number, rustJobs: RustJobs): ToolRunOutput {
  const { command, args } = toolInvocation(tool, tsconfig, rustJobs);
  const times: number[] = [];
  const footprint: Array<number | null> = [];
  const rss: Array<number | null> = [];
  let rssSource = 'unavailable';
  let lastOutput = { exitCode: 0 as number | null, stdout: '', stderr: '' };

  for (let i = 0; i < runs + warmup; i++) {
    const res = runMeasuredCommand(command, args, { cwd: workspaceRoot });
    if (i >= warmup) {
      times.push(res.durationMs);
      footprint.push(res.peakFootprintBytes);
      rss.push(res.peakRssBytes);
      if (res.peakRssSource !== 'unavailable') {
        rssSource = res.peakRssSource;
      }
    }
    lastOutput = { exitCode: res.status, stdout: res.stdout, stderr: res.stderr };
  }

  return { ...lastOutput, times, footprint, rss, rssSource };
}

function memoryStats(samples: number[], source: string): BenchMemoryStats {
  const sorted = [...samples].sort((a, b) => a - b);
  return {
    medianBytes: sorted[Math.floor(sorted.length / 2)],
    minBytes: sorted[0],
    maxBytes: sorted[sorted.length - 1],
    runs: sorted.length,
    source,
  };
}

function runBenchmark(
  tool: Tool,
  tsconfig: string,
  iterations: number,
  warmup: number,
  rustJobs: RustJobs,
): { stats: RunStats; memory: BenchMemoryStats | null } {
  const { times, footprint, rss, rssSource } = runTool(tool, tsconfig, iterations, warmup, rustJobs);
  times.sort((a, b) => a - b);
  const median = times[Math.floor(times.length / 2)] / 1000;
  const min = times[0] / 1000;
  const max = times[times.length - 1] / 1000;

  // Prefer phys_footprint (macOS, Activity-Monitor-comparable) over max RSS.
  const nonNull = (samples: Array<number | null>) =>
    samples.filter((sample): sample is number => sample !== null);
  const footprintSamples = nonNull(footprint);
  const rssSamples = nonNull(rss);
  const memory: BenchMemoryStats | null =
    footprintSamples.length > 0
      ? memoryStats(footprintSamples, 'phys_footprint')
      : rssSamples.length > 0
        ? memoryStats(rssSamples, `max_rss (${rssSource})`)
        : null;

  return { stats: { median, min, max, runs: iterations }, memory };
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

function printResults(results: BenchReportResult[]) {
  console.log('\nPerformance:');
  console.log(
    `project`.padEnd(30) +
      `tool`.padEnd(25) +
      `median`.padEnd(10) +
      `min`.padEnd(10) +
      `max`.padEnd(10) +
      `runs`.padEnd(7) +
      `vs tsc`.padEnd(9) +
      `peak mem`,
  );
  for (const r of results) {
    for (const tool of ['tsc', 'tsgo', 'tsgo-singleThreaded', 'surge-ts'] as Tool[]) {
      if (r.stats[tool]) {
        const s = r.stats[tool]!;
        const toolLabel = tool === 'surge-ts' ? `${toolDisplayLabel(tool)} (jobs=${r.rustJobs})` : toolDisplayLabel(tool);
        const speedup = speedupVsTsc(r, tool);
        const speedupLabel = speedup === null ? '—' : formatSpeedup(speedup).replace(' vs tsc', '');
        const memory = r.memory?.[tool];
        const rssLabel = memory ? formatBytes(memory.medianBytes) : '—';
        console.log(
          r.project.padEnd(30) +
            toolLabel.padEnd(25) +
            `${s.median.toFixed(2)}s`.padEnd(10) +
            `${s.min.toFixed(2)}s`.padEnd(10) +
            `${s.max.toFixed(2)}s`.padEnd(10) +
            String(s.runs).padEnd(7) +
            speedupLabel.padEnd(9) +
            rssLabel,
        );
      }
    }
  }

  console.log('\nDiagnostic drift:');
  console.log(`${`project`.padEnd(30) + `tool`.padEnd(25)}status`);
  for (const r of results) {
    for (const tool of ['tsgo', 'tsgo-singleThreaded', 'surge-ts'] as Tool[]) {
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
    rustJobs: 'auto',
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
      const value = argv[++i];
      parsed.rustJobs = value === 'auto' ? 'auto' : parseInt(value, 10);
    }
  }

  if (parsed.rustJobs !== 'auto' && (!Number.isInteger(parsed.rustJobs) || parsed.rustJobs <= 0)) {
    throw new Error('--rustJobs must be a positive integer or "auto"');
  }

  return parsed;
}

main();
