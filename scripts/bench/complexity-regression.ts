/// Complexity regression suite: generates synthetic projects at multiple
/// sizes, runs the release `surge` CLI with SURGE_TIMINGS=1, parses the
/// instrumentation counters from stderr, and classifies each tracked counter's
/// growth (constant / ~linear / superlinear) against a per-case expectation.
///
/// Counter proxies per case:
///   - shared-checker-options: `dependency_declaration_table_clone_count` and
///     `generated_default_lib_table_clone_count` must stay exactly 0 (those
///     tables are shared, never per-module-cloned) and
///     `type_declaration_table_clone_count` stays constant per run.
///   - serial-context-reuse (jobs=1): `symbol_table_clone_count` and
///     `module_export_table_clone_count` grow linearly with file count —
///     quadratic growth would mean per-file state is re-cloned per pass.
///   - union-scaling: `union_type_clone_count` / `union_type_payload_alloc_count`
///     grow linearly with union member work; `union_type_alloc_count` and
///     `union_type_payload_deep_clone_count` stay 0 on this path.
///   - overload-scaling: `overload_array_alloc_count` tracks the overload count;
///     `overload_group_create_count` stays constant.
///   - inheritance-scaling: `interface_member_declaration_visit_count` and
///     `interface_own_property_map_alloc_count` grow linearly with chain
///     depth/base width.
///
/// Exit code is non-zero when a zero-expected counter becomes nonzero, a
/// constant/linear-expected counter classifies as superlinear, a synthetic
/// project unexpectedly produces diagnostics, or a determinism check fails.
/// Wall time is displayed but never gated.
///
/// Usage:
///   pnpm bench:complexity [-- --json] [--skipBuild] [--binary <path>]
///     [--case <substring>] [--sizes 64,128,256] [--genDir <dir>]

import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  generateInheritanceProject,
  generateModuleGraphProject,
  generateOverloadProject,
  generateUnionProject,
} from './complexity-gen.js';

const scriptPath = fileURLToPath(import.meta.url);
const workspaceRoot = path.resolve(path.dirname(scriptPath), '../..');

export type GrowthClass = 'zero' | 'constant' | 'linear' | 'superlinear';
export type Expectation = 'zero' | 'constant' | 'linear' | 'known-superlinear';

export type CounterCaseSpec = {
  label: string;
  counter: string;
  expected: Expectation;
};

export type ProjectSpec = {
  name: string;
  sizes: number[];
  generate: (dir: string, n: number) => string;
  counterCases: CounterCaseSpec[];
};

export type CounterCaseResult = CounterCaseSpec & {
  project: string;
  sizes: number[];
  totals: number[];
  classification: GrowthClass;
  tailExponent: number | null;
  fitExponent: number | null;
  pass: boolean;
  note: string;
};

export type DeterminismResult = {
  name: string;
  status: 'pass' | 'fail' | 'skipped';
  note: string;
};

/// Counter lines are rendered by metrics/timings.rs as `    name: value` under
/// the `  counters:` header; the earlier `io:`/`file_metrics:` sections and the
/// RSS block must not be picked up.
export function parseTimingsCounters(stderr: string): Map<string, number> {
  const counters = new Map<string, number>();
  let inCounters = false;
  for (const line of stderr.split('\n')) {
    if (/^\s{2}counters:\s*$/.test(line)) {
      inCounters = true;
      continue;
    }
    if (!inCounters) {
      continue;
    }
    const match = /^ {4}([a-zA-Z_][a-zA-Z0-9_]*): (-?\d+(?:\.\d+)?)$/.exec(line);
    if (match) {
      counters.set(match[1], Number(match[2]));
    } else if (!line.startsWith('    ')) {
      inCounters = false;
    }
  }
  return counters;
}

function leastSquaresSlope(xs: number[], ys: number[]): number {
  const count = xs.length;
  const meanX = xs.reduce((a, b) => a + b, 0) / count;
  const meanY = ys.reduce((a, b) => a + b, 0) / count;
  let numerator = 0;
  let denominator = 0;
  for (let i = 0; i < count; i += 1) {
    numerator += (xs[i] - meanX) * (ys[i] - meanY);
    denominator += (xs[i] - meanX) ** 2;
  }
  return denominator === 0 ? 0 : numerator / denominator;
}

export const CONSTANT_TAIL_EXPONENT = 0.3;
export const LINEAR_TAIL_EXPONENT = 1.45;

/// Tail exponent (last two sizes) is the classifier: a fixed offset plus an
/// O(n^p) term converges to p at the tail, while a whole-series fit would be
/// dragged down by the offset. The +1 smoothing keeps zero totals finite.
export function classifyGrowth(
  sizes: number[],
  totals: number[],
): { classification: GrowthClass; tailExponent: number | null; fitExponent: number | null } {
  if (sizes.length !== totals.length || sizes.length < 2) {
    throw new Error('classifyGrowth requires matching sizes/totals with at least two points');
  }
  if (totals.every((total) => total === 0)) {
    return { classification: 'zero', tailExponent: null, fitExponent: null };
  }
  const last = totals.length - 1;
  const tailExponent =
    Math.log((totals[last] + 1) / (totals[last - 1] + 1)) /
    Math.log(sizes[last] / sizes[last - 1]);
  const fitExponent = leastSquaresSlope(
    sizes.map((n) => Math.log(n)),
    totals.map((total) => Math.log(total + 1)),
  );
  let classification: GrowthClass;
  if (tailExponent < CONSTANT_TAIL_EXPONENT) {
    classification = 'constant';
  } else if (tailExponent <= LINEAR_TAIL_EXPONENT) {
    classification = 'linear';
  } else {
    classification = 'superlinear';
  }
  return { classification, tailExponent, fitExponent };
}

export function evaluateExpectation(
  expected: Expectation,
  totals: number[],
  classification: GrowthClass,
): { pass: boolean; note: string } {
  switch (expected) {
    case 'zero':
      return totals.every((total) => total === 0)
        ? { pass: true, note: 'all zero' }
        : { pass: false, note: `expected 0 at every size, got [${totals.join(', ')}]` };
    case 'constant':
      if (classification === 'superlinear') {
        return { pass: false, note: 'expected constant, measured superlinear' };
      }
      return {
        pass: true,
        note: classification === 'linear' ? 'WARN: expected constant, measured linear' : 'ok',
      };
    case 'linear':
      return classification === 'superlinear'
        ? { pass: false, note: 'expected at most linear, measured superlinear' }
        : { pass: true, note: 'ok' };
    case 'known-superlinear':
      return { pass: true, note: 'known-superlinear (reported, not gated)' };
  }
}

export function sha256(text: string): string {
  return createHash('sha256').update(text).digest('hex');
}

export function projectSpecs(): ProjectSpec[] {
  return [
    {
      name: 'shared-checker-options',
      sizes: [64, 128, 256, 512],
      generate: generateModuleGraphProject,
      counterCases: [
        {
          label: 'dependency decl table clones',
          counter: 'dependency_declaration_table_clone_count',
          expected: 'zero',
        },
        {
          label: 'default-lib table clones',
          counter: 'generated_default_lib_table_clone_count',
          expected: 'zero',
        },
        {
          label: 'type decl table clones',
          counter: 'type_declaration_table_clone_count',
          expected: 'constant',
        },
        {
          label: 'module analysis calls',
          counter: 'module_analysis_total_calls',
          expected: 'linear',
        },
      ],
    },
    {
      name: 'serial-context-reuse',
      sizes: [64, 128, 256, 512],
      generate: generateModuleGraphProject,
      counterCases: [
        {
          label: 'symbol table clones',
          counter: 'symbol_table_clone_count',
          expected: 'linear',
        },
        {
          label: 'module export table clones',
          counter: 'module_export_table_clone_count',
          expected: 'linear',
        },
        {
          label: 'expression checks',
          counter: 'expression_check_count',
          expected: 'linear',
        },
      ],
    },
    {
      name: 'union-scaling',
      sizes: [64, 128, 256, 512, 1024],
      generate: generateUnionProject,
      counterCases: [
        {
          label: 'union member work',
          counter: 'union_type_clone_count',
          expected: 'linear',
        },
        {
          label: 'union payload allocs',
          counter: 'union_type_payload_alloc_count',
          expected: 'linear',
        },
        {
          label: 'eager union allocs',
          counter: 'union_type_alloc_count',
          expected: 'zero',
        },
        {
          label: 'union payload deep clones',
          counter: 'union_type_payload_deep_clone_count',
          expected: 'zero',
        },
      ],
    },
    {
      name: 'overload-scaling',
      sizes: [32, 64, 128, 256],
      generate: generateOverloadProject,
      counterCases: [
        {
          label: 'overload work',
          counter: 'overload_array_alloc_count',
          expected: 'linear',
        },
        {
          label: 'overload group creates',
          counter: 'overload_group_create_count',
          expected: 'constant',
        },
        {
          label: 'interface member visits',
          counter: 'interface_member_declaration_visit_count',
          expected: 'linear',
        },
        {
          label: 'expression checks',
          counter: 'expression_check_count',
          expected: 'linear',
        },
      ],
    },
    {
      name: 'inheritance-scaling',
      sizes: [32, 64, 128, 256],
      generate: generateInheritanceProject,
      counterCases: [
        {
          label: 'interface member visits',
          counter: 'interface_member_declaration_visit_count',
          expected: 'linear',
        },
        {
          label: 'property map allocs',
          counter: 'interface_own_property_map_alloc_count',
          expected: 'linear',
        },
        {
          label: 'property lookups (fixed probes)',
          counter: 'property_lookup_count',
          expected: 'constant',
        },
        {
          label: 'object payload deep clones',
          counter: 'object_type_payload_deep_clone_count',
          expected: 'zero',
        },
      ],
    },
  ];
}

export type ParsedArgs = {
  json: boolean;
  skipBuild: boolean;
  binary: string;
  caseFilter: string | null;
  sizesOverride: number[] | null;
  genDir: string;
};

export function parseArgs(argv: string[]): ParsedArgs {
  const parsed: ParsedArgs = {
    json: false,
    skipBuild: false,
    binary: path.join(
      workspaceRoot,
      'target',
      'release',
      process.platform === 'win32' ? 'surge.exe' : 'surge',
    ),
    caseFilter: null,
    sizesOverride: null,
    genDir: path.join(workspaceRoot, 'target', 'complexity-gen'),
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const next = () => {
      i += 1;
      const value = argv[i];
      if (value === undefined) {
        throw new Error(`missing value for ${arg}`);
      }
      return value;
    };
    switch (arg) {
      // pnpm forwards the `--` separator literally; ignore it.
      case '--':
        break;
      case '--json':
        parsed.json = true;
        break;
      case '--skipBuild':
        parsed.skipBuild = true;
        break;
      case '--binary':
        parsed.binary = path.resolve(next());
        break;
      case '--case':
        parsed.caseFilter = next();
        break;
      case '--sizes': {
        const sizes = next()
          .split(',')
          .map((value) => Number(value.trim()));
        if (sizes.length < 2 || sizes.some((n) => !Number.isInteger(n) || n < 2)) {
          throw new Error('--sizes must be at least two integers >= 2');
        }
        parsed.sizesOverride = sizes;
        break;
      }
      case '--genDir':
        parsed.genDir = path.resolve(next());
        break;
      default:
        throw new Error(`unknown argument: ${arg}`);
    }
  }
  return parsed;
}

type CounterRun = {
  counters: Map<string, number>;
  wallMs: number;
};

function runCounterCheck(binary: string, tsconfigPath: string): CounterRun {
  const start = performance.now();
  const result = spawnSync(
    binary,
    ['--project', tsconfigPath, '--format', 'json', '--maxDiagnostics', '10000', '--jobs', '1'],
    {
      cwd: workspaceRoot,
      encoding: 'utf8',
      env: { ...process.env, SURGE_TIMINGS: '1' },
      maxBuffer: 256 * 1024 * 1024,
    },
  );
  const wallMs = performance.now() - start;
  if (result.status !== 0) {
    let detail = result.stderr.slice(0, 1500);
    try {
      const parsed = JSON.parse(result.stdout) as { diagnostics?: unknown[] };
      if (Array.isArray(parsed.diagnostics) && parsed.diagnostics.length > 0) {
        detail = `synthetic project produced ${parsed.diagnostics.length} diagnostics; counters would measure the wrong code path. First: ${JSON.stringify(parsed.diagnostics[0])}`;
      }
    } catch {
      // fall through to stderr detail
    }
    throw new Error(`surge exited with ${result.status ?? result.signal} for ${tsconfigPath}: ${detail}`);
  }
  const counters = parseTimingsCounters(result.stderr);
  if (counters.size === 0) {
    throw new Error(`no counters parsed from stderr for ${tsconfigPath}`);
  }
  return { counters, wallMs };
}

function runDeterminismPair(
  name: string,
  binary: string,
  tsconfigPath: string,
): DeterminismResult {
  const hashes: string[] = [];
  const exitCodes: Array<number | null> = [];
  for (let run = 0; run < 2; run += 1) {
    const result = spawnSync(
      binary,
      ['--project', tsconfigPath, '--format', 'json', '--maxDiagnostics', '10000', '--jobs', '1'],
      { cwd: workspaceRoot, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 },
    );
    if (result.status !== 0 && result.status !== 2) {
      return {
        name,
        status: 'fail',
        note: `run ${run + 1} exited with ${result.status ?? result.signal}`,
      };
    }
    hashes.push(sha256(result.stdout));
    exitCodes.push(result.status);
  }
  if (hashes[0] !== hashes[1] || exitCodes[0] !== exitCodes[1]) {
    return {
      name,
      status: 'fail',
      note: `stdout hashes differ: ${hashes[0].slice(0, 16)} vs ${hashes[1].slice(0, 16)} (exit ${exitCodes[0]}/${exitCodes[1]})`,
    };
  }
  return { name, status: 'pass', note: `sha256 ${hashes[0].slice(0, 16)}… twice (exit ${exitCodes[0]})` };
}

export function renderMarkdownReport(
  results: CounterCaseResult[],
  wallMs: Map<string, number[]>,
  determinism: DeterminismResult[],
): string {
  const lines: string[] = [];
  const byProject = new Map<string, CounterCaseResult[]>();
  for (const result of results) {
    const group = byProject.get(result.project) ?? [];
    group.push(result);
    byProject.set(result.project, group);
  }

  for (const [project, group] of byProject) {
    const sizes = group[0].sizes;
    lines.push(`## ${project}`);
    lines.push('');
    const header = ['Case', ...sizes.map((n) => `n=${n}`), 'Growth', 'Expected', 'Status'];
    lines.push(`| ${header.join(' | ')} |`);
    lines.push(`| ${header.map(() => '---').join(' | ')} |`);
    for (const result of group) {
      const growth =
        result.classification === 'zero'
          ? 'constant (zero)'
          : result.classification === 'linear'
            ? `~linear (p=${result.tailExponent!.toFixed(2)})`
            : `${result.classification} (p=${result.tailExponent!.toFixed(2)})`;
      lines.push(
        `| ${result.label} (\`${result.counter}\`) | ${result.totals.join(' | ')} | ${growth} | ${result.expected} | ${result.pass ? 'PASS' : 'FAIL'}${result.note !== 'ok' && result.note !== 'all zero' ? ` — ${result.note}` : ''} |`,
      );
    }
    const wall = wallMs.get(project);
    if (wall) {
      lines.push(
        `| wall ms (displayed, never gated) | ${wall.map((ms) => ms.toFixed(0)).join(' | ')} | — | — | — |`,
      );
    }
    lines.push('');
  }

  lines.push('## determinism');
  lines.push('');
  lines.push('| Check | Status | Note |');
  lines.push('| --- | --- | --- |');
  for (const result of determinism) {
    lines.push(`| ${result.name} | ${result.status.toUpperCase()} | ${result.note} |`);
  }
  lines.push('');
  return lines.join('\n');
}

function main(): void {
  const parsed = parseArgs(process.argv.slice(2));

  if (!parsed.skipBuild) {
    const build = spawnSync('cargo', ['build', '--release', '-p', 'surge-ts-cli'], {
      cwd: workspaceRoot,
      stdio: 'inherit',
    });
    if (build.status !== 0) {
      throw new Error('cargo build --release -p surge-ts-cli failed');
    }
  }
  if (!existsSync(parsed.binary)) {
    throw new Error(`missing surge binary at ${parsed.binary}; run without --skipBuild`);
  }

  let specs = projectSpecs();
  if (parsed.caseFilter !== null) {
    specs = specs.filter((spec) => spec.name.includes(parsed.caseFilter!));
    if (specs.length === 0) {
      throw new Error(`no case matches --case ${parsed.caseFilter}`);
    }
  }

  // Distinct project specs sharing a generator (shared-checker-options and
  // serial-context-reuse) reuse the same generated projects and CLI runs.
  const runCache = new Map<string, CounterRun>();
  const results: CounterCaseResult[] = [];
  const wallByProject = new Map<string, number[]>();

  for (const spec of specs) {
    const sizes = parsed.sizesOverride ?? spec.sizes;
    const runs: CounterRun[] = [];
    for (const n of sizes) {
      const key = `${spec.generate.name}:${n}`;
      let run = runCache.get(key);
      if (run === undefined) {
        const dir = path.join(parsed.genDir, spec.generate.name, String(n));
        rmSync(dir, { recursive: true, force: true });
        mkdirSync(dir, { recursive: true });
        const tsconfigPath = spec.generate(dir, n);
        run = runCounterCheck(parsed.binary, tsconfigPath);
        runCache.set(key, run);
      }
      runs.push(run);
    }
    wallByProject.set(
      spec.name,
      runs.map((run) => run.wallMs),
    );
    for (const counterCase of spec.counterCases) {
      const totals = runs.map((run) => {
        const value = run.counters.get(counterCase.counter);
        if (value === undefined) {
          throw new Error(`counter ${counterCase.counter} missing from timings output`);
        }
        return value;
      });
      const { classification, tailExponent, fitExponent } = classifyGrowth(sizes, totals);
      const { pass, note } = evaluateExpectation(counterCase.expected, totals, classification);
      results.push({
        ...counterCase,
        project: spec.name,
        sizes,
        totals,
        classification,
        tailExponent,
        fitExponent,
        pass,
        note,
      });
    }
  }

  const determinism: DeterminismResult[] = [];
  if (parsed.caseFilter === null || 'determinism'.includes(parsed.caseFilter)) {
    determinism.push(
      runDeterminismPair(
        'zod-shaped fixture (2 fresh processes)',
        parsed.binary,
        path.join(
          workspaceRoot,
          'tests',
          'compat-projects',
          'complexity-zod-shaped-determinism',
          'tsconfig.json',
        ),
      ),
    );
    const realZod = path.join(workspaceRoot, '.local-projects', 'zod', 'tsconfig.json');
    if (existsSync(realZod)) {
      determinism.push(runDeterminismPair('real zod (2 fresh processes)', parsed.binary, realZod));
    } else {
      determinism.push({
        name: 'real zod (2 fresh processes)',
        status: 'skipped',
        note: '.local-projects/zod not present',
      });
    }
  }

  const pass =
    results.every((result) => result.pass) &&
    determinism.every((result) => result.status !== 'fail');

  if (parsed.json) {
    console.log(
      JSON.stringify(
        {
          timestamp: new Date().toISOString(),
          binary: parsed.binary,
          pass,
          results,
          wallMs: Object.fromEntries(wallByProject),
          determinism,
        },
        null,
        2,
      ),
    );
  } else {
    console.log(renderMarkdownReport(results, wallByProject, determinism));
    console.log(pass ? 'complexity regression suite: PASS' : 'complexity regression suite: FAIL');
  }
  if (!pass) {
    process.exitCode = 1;
  }
}

const isMain = Boolean(process.argv[1]) && path.resolve(process.argv[1]) === scriptPath;
if (isMain) {
  main();
}
