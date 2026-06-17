#!/usr/bin/env tsx

import { type SpawnSyncReturns, spawnSync } from 'node:child_process';
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import os from 'node:os';
import { performance } from 'node:perf_hooks';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { toolDisplayLabel } from '../bench/report.js';

export type DiagnosticFingerprint = {
  fileName: string;
  code: string;
  line: number | null;
  column: number | null;
  message: string | null;
  count: number;
};

export type CountBucket = {
  key: string;
  typescript: number;
  surgeTs: number;
};

export type OracleComparison = {
  project: string | null;
  warnings?: string[];
  typescript: {
    total: number;
    byCode: Array<{ key: string; count: number }>;
  };
  surgeTs: {
    total: number;
  };
  matches?: {
    onlyTypeScriptFileCodeLine?: CountBucket[];
    onlySurgeTsFileCodeLine?: CountBucket[];
  };
  summary: {
    byCodeMatch: boolean;
    byFileCodeMatch: boolean;
    byFileCodeLineMatch: boolean | null;
  };
  details?: {
    onlyTypeScript?: {
      rawDiagnosticFingerprints?: DiagnosticFingerprint[];
    };
    onlySurgeTs?: {
      rawDiagnosticFingerprints?: DiagnosticFingerprint[];
    };
  };
};

export type CompatReport = {
  filesLoaded: number;
  loadedSourceFiles: number;
  loadedRootDeclarationFiles: number;
  loadedDependencyDeclarationFiles: number;
  loadedGeneratedDeclarationFiles: number;
  diagnosticsDependencyDeclarationTotal: number;
  suppressedRustOnlyDiagnosticsTotal: number;
};

export type BenchStats = {
  median: number;
  min: number;
  max: number;
  runs: number;
};

export type RustJobValue = number | 'auto';

export type BenchResult = {
  project: string;
  rustJobs?: RustJobValue;
  stats: Record<string, BenchStats | null>;
  drift: Record<string, string>;
};

export type ProgramMeasurements = {
  timings: Map<string, string>;
  counters: Map<string, string>;
};

export type ResolvedProject = {
  root: string;
  tsconfig: string;
  attempted: string[];
};

export type ParsedArgs = {
  project: string | null;
  name: string | null;
  maxDiagnostics: number;
  rustJobs: RustJobValue[];
  outDir: string | null;
  allowMissing: boolean;
  authKitFallback: boolean;
};

export type JobOutputPaths = {
  jobs: RustJobValue;
  json: string;
  html: string;
  svg: string;
};

export type OutputPaths = {
  outDir: string;
  oracleCompareTxt: string;
  oracleCompareJson: string;
  compatReportJson: string;
  timingsTxt: string;
  measurementMd: string;
  memoryJson: string;
  jobs: JobOutputPaths[];
};

/// How a command's peak resident set size was obtained. `macos-time` /
/// `linux-time` mean `/usr/bin/time` produced a parseable value; `unavailable`
/// means peak RSS could not be measured (no `/usr/bin/time`, unsupported
/// platform, or an unparseable report), in which case the command still runs.
export type PeakRssSource = 'macos-time' | 'linux-time' | 'unavailable';

export type MeasuredCommandResult = {
  command: string;
  args: string[];
  status: number | null;
  signal: NodeJS.Signals | null;
  stdout: string;
  stderr: string;
  durationMs: number;
  peakRssBytes: number | null;
  peakRssSource: PeakRssSource;
  error: string | null;
};

export type RunMeasuredOptions = {
  cwd?: string;
  env?: NodeJS.ProcessEnv;
  maxBuffer?: number;
  platform?: NodeJS.Platform;
  spawn?: typeof spawnSync;
  now?: () => number;
  timeBinaryPath?: string;
  timeBinaryExists?: (binaryPath: string) => boolean;
  makeReportPath?: () => string;
  readReport?: (reportPath: string) => string | null;
};

/// One peak-RSS sample for a single compiler invocation. The snake_case JSON
/// shape (`peak_rss_bytes`, etc.) is what lands in the `memory.json` artifact.
export type MemoryModeResult = {
  command: string;
  mode: string;
  format: string;
  compatReport: boolean;
  timings: boolean;
  rustJobs: RustJobValue | null;
  peakRssBytes: number | null;
  peakRssMb: number | null;
  peakRssSource: PeakRssSource;
  durationMs: number;
  status: number | null;
};

const DEFAULT_TIME_BINARY = '/usr/bin/time';
const DEFAULT_MEASURED_MAX_BUFFER = 256 * 1024 * 1024;
let reportPathCounter = 0;

export function timeMeasurementForPlatform(
  platform: NodeJS.Platform,
): { flag: string; source: Exclude<PeakRssSource, 'unavailable'> } | null {
  if (platform === 'darwin') {
    return { flag: '-l', source: 'macos-time' };
  }
  if (platform === 'linux') {
    return { flag: '-v', source: 'linux-time' };
  }
  return null;
}

/// Parse the peak resident set size (in bytes) out of a `/usr/bin/time` report.
/// macOS BSD `time -l` reports the value in bytes; Linux GNU `time -v` reports
/// kibibytes, so it is scaled to bytes. The matched field labels are stable
/// across locales for both implementations, so no locale handling is needed.
export function parsePeakRssBytes(report: string, source: PeakRssSource): number | null {
  if (source === 'macos-time') {
    const match = report.match(/(\d+)\s+maximum resident set size/i);
    return match ? Number(match[1]) : null;
  }
  if (source === 'linux-time') {
    const match = report.match(/maximum resident set size\s*\(kbytes\):\s*(\d+)/i);
    return match ? Number(match[1]) * 1024 : null;
  }
  return null;
}

export function peakRssMb(bytes: number | null): number | null {
  return bytes === null ? null : Math.round((bytes / (1024 * 1024)) * 10) / 10;
}

function defaultReportPath(): string {
  reportPathCounter += 1;
  return path.join(os.tmpdir(), `ts-rust-rss-${process.pid}-${reportPathCounter}.txt`);
}

function defaultReadReport(reportPath: string): string | null {
  try {
    return readFileSync(reportPath, 'utf8');
  } catch {
    return null;
  } finally {
    try {
      unlinkSync(reportPath);
    } catch {
      // The report file may not exist if the command never started; ignore.
    }
  }
}

/// Run `command` under `/usr/bin/time` and capture its peak RSS alongside the
/// child's stdout/stderr. The time report is written to a dedicated `-o` file
/// (supported by both BSD and GNU `time`), so the child's stderr stays clean —
/// callers that parse `--timings` output from stderr are unaffected. When
/// `/usr/bin/time` is unavailable or the platform is unsupported, the command
/// still runs and `peakRssSource` is `unavailable`; a failed child is reported
/// (non-zero `status`/`error`) with its memory still parsed when possible.
export function runMeasuredCommand(
  command: string,
  args: string[],
  options: RunMeasuredOptions = {},
): MeasuredCommandResult {
  const platform = options.platform ?? process.platform;
  const spawn = options.spawn ?? spawnSync;
  const now = options.now ?? (() => performance.now());
  const timeBinary = options.timeBinaryPath ?? DEFAULT_TIME_BINARY;
  const timeExists = options.timeBinaryExists ?? ((binaryPath: string) => existsSync(binaryPath));

  const spawnOptions = {
    cwd: options.cwd,
    encoding: 'utf8' as const,
    maxBuffer: options.maxBuffer ?? DEFAULT_MEASURED_MAX_BUFFER,
    env: options.env ?? process.env,
  };

  const toResult = (
    result: SpawnSyncReturns<string>,
    durationMs: number,
    peakRssBytes: number | null,
    peakRssSource: PeakRssSource,
  ): MeasuredCommandResult => ({
    command,
    args,
    status: result.status,
    signal: result.signal ?? null,
    stdout: `${result.stdout ?? ''}`,
    stderr: `${result.stderr ?? ''}`,
    durationMs,
    peakRssBytes,
    peakRssSource,
    error: result.error ? result.error.message : null,
  });

  const measurement = timeMeasurementForPlatform(platform);
  if (measurement === null || !timeExists(timeBinary)) {
    const start = now();
    const result = spawn(command, args, spawnOptions) as SpawnSyncReturns<string>;
    return toResult(result, now() - start, null, 'unavailable');
  }

  const reportPath = options.makeReportPath ? options.makeReportPath() : defaultReportPath();
  const readReport = options.readReport ?? defaultReadReport;

  const start = now();
  const result = spawn(timeBinary, [measurement.flag, '-o', reportPath, command, ...args], spawnOptions) as SpawnSyncReturns<string>;
  const durationMs = now() - start;

  const report = readReport(reportPath);
  const peakRssBytes = report !== null ? parsePeakRssBytes(report, measurement.source) : null;
  return toResult(
    result,
    durationMs,
    peakRssBytes,
    peakRssBytes !== null ? measurement.source : 'unavailable',
  );
}

const scriptPath = fileURLToPath(import.meta.url);
const scriptDir = path.dirname(scriptPath);
const workspaceRoot = path.resolve(scriptDir, '../..');
const benchDir = path.join(workspaceRoot, '.bench');

const DEFAULT_MAX_DIAGNOSTICS = 500;
const DEFAULT_RUST_JOBS: RustJobValue[] = [1, 'auto'];

export function parseArgs(argv: string[]): ParsedArgs {
  const parsed: ParsedArgs = {
    project: null,
    name: null,
    maxDiagnostics: DEFAULT_MAX_DIAGNOSTICS,
    rustJobs: [...DEFAULT_RUST_JOBS],
    outDir: null,
    allowMissing: false,
    authKitFallback: false,
  };

  const requireValue = (flag: string, value: string | undefined): string => {
    if (value === undefined) {
      throw new Error(`Missing value for ${flag}`);
    }
    return value;
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    switch (arg) {
      case '--':
        break;
      case '--project':
        parsed.project = requireValue(arg, argv[(i += 1)]);
        break;
      case '--name':
        parsed.name = requireValue(arg, argv[(i += 1)]);
        break;
      case '--maxDiagnostics': {
        const value = Number(requireValue(arg, argv[(i += 1)]));
        if (!Number.isInteger(value) || value <= 0) {
          throw new Error(`--maxDiagnostics must be a positive integer, got "${argv[i]}"`);
        }
        parsed.maxDiagnostics = value;
        break;
      }
      case '--rustJobs': {
        const value = requireValue(arg, argv[(i += 1)]);
        parsed.rustJobs = parseRustJobs(value);
        break;
      }
      case '--outDir':
        parsed.outDir = requireValue(arg, argv[(i += 1)]);
        break;
      case '--allowMissing':
        parsed.allowMissing = true;
        break;
      case '--authKitFallback':
        parsed.authKitFallback = true;
        break;
      default:
        throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return parsed;
}

export function parseRustJobs(value: string): RustJobValue[] {
  const parts = value
    .split(',')
    .map((part) => part.trim())
    .filter((part) => part.length > 0);

  if (parts.length === 0) {
    throw new Error(`--rustJobs must be a comma-separated list of positive integers or "auto", got "${value}"`);
  }

  const jobs: RustJobValue[] = parts.map((part) => {
    if (part === 'auto') {
      return 'auto';
    }
    const n = Number(part);
    if (!Number.isInteger(n) || n <= 0) {
      throw new Error(`--rustJobs must be a comma-separated list of positive integers or "auto", got "${value}"`);
    }
    return n;
  });

  const seen = new Set<RustJobValue>();
  return jobs.filter((job) => {
    if (seen.has(job)) return false;
    seen.add(job);
    return true;
  });
}

export function authKitCandidateRoots(root: string, env: NodeJS.ProcessEnv): string[] {
  return [
    env.AUTH_KIT_PROJECT,
    path.resolve(root, '../../typescript/auth-project/auth-kit'),
    path.resolve(root, '.local-projects/auth-kit'),
  ].filter((value): value is string => Boolean(value));
}

type PathKind = 'file' | 'dir' | 'missing';

function defaultClassify(target: string): PathKind {
  try {
    return statSync(target).isDirectory() ? 'dir' : 'file';
  } catch {
    return 'missing';
  }
}

export function resolveProject(
  opts: { project: string | null; authKitFallback: boolean },
  deps: {
    workspaceRoot: string;
    cwd?: string;
    candidateRoots?: string[];
    classify?: (target: string) => PathKind;
  },
): ResolvedProject | null {
  const classify = deps.classify ?? defaultClassify;
  const cwd = deps.cwd ?? process.cwd();
  const attempted: string[] = [];

  if (opts.project) {
    const abs = path.isAbsolute(opts.project) ? opts.project : path.resolve(cwd, opts.project);
    attempted.push(abs);
    const kind = classify(abs);
    if (kind === 'file') {
      return { root: path.dirname(abs), tsconfig: abs, attempted };
    }
    if (kind === 'dir') {
      const tsconfig = path.join(abs, 'tsconfig.json');
      attempted.push(tsconfig);
      if (classify(tsconfig) === 'file') {
        return { root: abs, tsconfig, attempted };
      }
    }
    return null;
  }

  if (opts.authKitFallback) {
    const candidates =
      deps.candidateRoots ?? authKitCandidateRoots(deps.workspaceRoot, process.env);
    for (const candidate of candidates) {
      const root = path.isAbsolute(candidate)
        ? candidate
        : path.resolve(deps.workspaceRoot, candidate);
      attempted.push(root);

      const tsconfig = path.join(root, 'tsconfig.json');
      if (classify(tsconfig) === 'file') {
        return { root, tsconfig, attempted };
      }

      if (path.basename(root).toLowerCase() === 'tsconfig.json' && classify(root) === 'file') {
        return { root: path.dirname(root), tsconfig: root, attempted };
      }
    }
  }

  return null;
}

export function projectNameFromPath(rootPath: string): string {
  return slugify(path.basename(rootPath));
}

export function slugify(value: string): string {
  const slug = value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return slug.length > 0 ? slug : 'project';
}

export function outputPathsForProject(outDir: string, rustJobs: RustJobValue[]): OutputPaths {
  return {
    outDir,
    oracleCompareTxt: path.join(outDir, 'oracle-compare.txt'),
    oracleCompareJson: path.join(outDir, 'oracle-compare.json'),
    compatReportJson: path.join(outDir, 'compat-report.json'),
    timingsTxt: path.join(outDir, 'timings.txt'),
    measurementMd: path.join(outDir, 'measurement.md'),
    memoryJson: path.join(outDir, 'memory.json'),
    jobs: rustJobs.map((jobs) => ({
      jobs,
      json: path.join(outDir, `jobs${jobs}.json`),
      html: path.join(outDir, `jobs${jobs}.html`),
      svg: path.join(outDir, `jobs${jobs}.svg`),
    })),
  };
}

function main(argv = process.argv.slice(2)): void {
  const args = parseArgs(argv);
  const project = resolveProject(args, { workspaceRoot });

  if (!project) {
    handleMissingProject(args, argv);
    return;
  }

  const name = args.name ? slugify(args.name) : projectNameFromPath(project.root);
  const outDir = args.outDir
    ? path.resolve(process.cwd(), args.outDir)
    : path.join(benchDir, 'real-projects', name);
  const outputs = outputPathsForProject(outDir, args.rustJobs);
  mkdirSync(outDir, { recursive: true });

  const commandUsed = `tsx scripts/real-projects/measure-project.ts ${argv.join(' ')}`.trim();

  const oracleText = runCommand('pnpm', [
    'run',
    'oracle:compare',
    '--',
    '--project',
    project.tsconfig,
    '--maxDiagnostics',
    String(args.maxDiagnostics),
  ]);
  writeFileSync(outputs.oracleCompareTxt, oracleText);

  const oracleJsonText = runCommand('pnpm', [
    'run',
    'oracle:compare',
    '--',
    '--project',
    project.tsconfig,
    '--maxDiagnostics',
    String(args.maxDiagnostics),
    '--json',
  ]);
  const oracleJson = extractJsonText(oracleJsonText, 'oracle:compare --json');
  writeFileSync(outputs.oracleCompareJson, `${oracleJson}\n`);
  const oracle = JSON.parse(oracleJson) as OracleComparison;

  runCommand('cargo', ['build', '--release', '-p', 'surge-ts-cli']);

  const binary = releaseBinaryPath();
  const memoryResults: MemoryModeResult[] = [];

  // The compatReport+timings run drives the compat-report and timing sections
  // and doubles as the peak-RSS sample for that mode. `/usr/bin/time` writes its
  // report to a separate file, so this command's stderr is still just the
  // program's `--timings` output and stays parseable below.
  const compatReportResult = runMeasuredCommand(
    binary,
    ['--project', project.tsconfig, '--compatReport', '--format', 'json', '--timings'],
    { cwd: workspaceRoot, env: measuredEnv() },
  );
  assertMeasuredOk(compatReportResult);
  memoryResults.push(
    toMemoryModeResult(compatReportResult, {
      command: 'surge-ts',
      mode: 'compatReport json + timings',
      format: 'json',
      compatReport: true,
      timings: true,
      rustJobs: 1,
    }),
  );
  const compatReportJson = extractJsonText(compatReportResult.stdout, 'compatReport --format json');
  writeFileSync(outputs.compatReportJson, `${compatReportJson}\n`);
  const compatReport = JSON.parse(compatReportJson) as CompatReport;
  writeFileSync(outputs.timingsTxt, compatReportResult.stderr);
  const programMeasurements = parseProgramMeasurements(compatReportResult.stderr);

  // Separate the remaining output modes so the report can attribute the peak to
  // a code path: core checking (default), JSON serialization, or compat-report
  // construction. jobs>1 variants expose any per-worker context duplication.
  memoryResults.push(
    measureRustMode(binary, project.tsconfig, [], {
      command: 'surge-ts',
      mode: 'default tsc renderer',
      format: 'text',
      compatReport: false,
      timings: false,
      rustJobs: 1,
    }),
    measureRustMode(binary, project.tsconfig, ['--format', 'json'], {
      command: 'surge-ts',
      mode: 'json',
      format: 'json',
      compatReport: false,
      timings: false,
      rustJobs: 1,
    }),
    measureRustMode(binary, project.tsconfig, ['--compatReport', '--format', 'json'], {
      command: 'surge-ts',
      mode: 'compatReport json',
      format: 'json',
      compatReport: true,
      timings: false,
      rustJobs: 1,
    }),
  );
  for (const job of args.rustJobs) {
    if (job === 1) {
      continue; // jobs=1 is already covered by the plain `json` mode.
    }
    memoryResults.push(
      measureRustMode(binary, project.tsconfig, ['--format', 'json', '--jobs', String(job)], {
        command: 'surge-ts',
        mode: `json (jobs=${job})`,
        format: 'json',
        compatReport: false,
        timings: false,
        rustJobs: job,
      }),
    );
  }

  // Baseline compilers are best-effort: invoked through their direct entry
  // binaries so `/usr/bin/time` measures the compiler process itself. Failures
  // (missing binary, parse issue) are recorded as unavailable, not fatal.
  memoryResults.push(...baselineCompilerMemory(project.tsconfig));

  writeFileSync(outputs.memoryJson, `${JSON.stringify(memoryResults.map(toMemoryJson), null, 2)}\n`);

  const benchResults: Array<{ jobs: RustJobValue; result: BenchResult }> = [];
  for (const job of outputs.jobs) {
    runCommand('pnpm', [
      'exec',
      'tsx',
      path.join(scriptDir, '../bench/compare-compilers.ts'),
      '--',
      '--project',
      project.tsconfig,
      '--json',
      job.json,
      '--html',
      job.html,
      '--chart',
      job.svg,
      '--rustJobs',
      String(job.jobs),
    ]);
    benchResults.push({ jobs: job.jobs, result: readBenchResult(job.json) });
  }

  const markdown = renderMeasurementMarkdown({
    name,
    project,
    timestamp: new Date().toISOString(),
    commandUsed,
    maxDiagnostics: args.maxDiagnostics,
    oracle,
    compatReport,
    benchResults,
    programMeasurements,
    memoryResults,
  });
  writeFileSync(outputs.measurementMd, markdown);

  if (name === 'auth-kit') {
    writeAuthKitCompatCopies(outputs);
  }

  process.stdout.write(`Wrote measurement report to ${outputs.measurementMd}\n`);
}

function handleMissingProject(args: ParsedArgs, argv: string[]): void {
  const project = resolveProject(args, { workspaceRoot });
  const attempted = project?.attempted ?? collectAttemptedPaths(args);
  const name = args.name ? slugify(args.name) : 'unresolved-project';
  const outDir = args.outDir
    ? path.resolve(process.cwd(), args.outDir)
    : path.join(benchDir, 'real-projects', name);
  mkdirSync(outDir, { recursive: true });

  const message = [
    `# ${name} Measurement (unavailable)`,
    '',
    'No project could be resolved, so no measurement was produced.',
    '',
    `- timestamp: ${new Date().toISOString()}`,
    `- command used: \`tsx scripts/real-projects/measure-project.ts ${argv.join(' ')}\``,
    `- allowMissing: ${boolToYesNo(args.allowMissing)}`,
    '',
    '## Attempted Paths',
    ...(attempted.length > 0 ? attempted.map((entry) => `- ${entry}`) : ['- none']),
    '',
  ].join('\n');

  const measurementMd = path.join(outDir, 'measurement.md');
  writeFileSync(measurementMd, `${message}\n`);
  if (name === 'auth-kit') {
    writeFileSync(path.join(benchDir, 'auth-kit-measurement.md'), `${message}\n`);
  }
  process.stdout.write(`${message}\n`);

  process.exitCode = args.allowMissing ? 0 : 1;
}

function collectAttemptedPaths(args: ParsedArgs): string[] {
  if (args.project) {
    return [
      path.isAbsolute(args.project)
        ? args.project
        : path.resolve(process.cwd(), args.project),
    ];
  }
  if (args.authKitFallback) {
    return authKitCandidateRoots(workspaceRoot, process.env).map((candidate) =>
      path.isAbsolute(candidate) ? candidate : path.resolve(workspaceRoot, candidate),
    );
  }
  return [];
}

function writeAuthKitCompatCopies(outputs: OutputPaths): void {
  mkdirSync(benchDir, { recursive: true });
  copyFileSync(outputs.measurementMd, path.join(benchDir, 'auth-kit-measurement.md'));
  copyFileSync(outputs.oracleCompareTxt, path.join(benchDir, 'auth-kit-oracle-compare.txt'));
  for (const job of outputs.jobs) {
    if (job.jobs === 1 && existsSync(job.json)) {
      copyFileSync(job.json, path.join(benchDir, 'auth-kit-jobs1.json'));
    }
    if (job.jobs === 4 && existsSync(job.json)) {
      copyFileSync(job.json, path.join(benchDir, 'auth-kit-jobs4.json'));
    }
  }
}

function runCommand(command: string, args: string[]): string {
  return runCommandResult(command, args).stdout;
}

function runCommandResult(command: string, args: string[]): { stdout: string; stderr: string } {
  const result = spawnSync(command, args, {
    cwd: workspaceRoot,
    encoding: 'utf8',
    maxBuffer: 50 * 1024 * 1024,
    env: {
      ...process.env,
      npm_config_cache: process.env.npm_config_cache ?? path.join(os.tmpdir(), 'npm-cache'),
    },
  });

  if (result.error) {
    throw new Error(`${command} failed: ${result.error.message}`);
  }

  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(' ')} exited with ${result.status ?? 'unknown'}:\n${result.stdout ?? ''}${result.stderr ?? ''}`,
    );
  }

  return {
    stdout: `${result.stdout ?? ''}`,
    stderr: `${result.stderr ?? ''}`,
  };
}

export function extractJsonText(raw: string, context: string): string {
  const text = raw.trim();
  try {
    JSON.parse(text);
    return text;
  } catch {
    // stdout may be prefixed/suffixed by pnpm reporter noise ("Already up to
    // date", "Done in ...") or other wrapper output; recover the JSON region.
  }

  for (let start = 0; start < text.length; start += 1) {
    const ch = text[start];
    if (ch !== '{' && ch !== '[') {
      continue;
    }
    const candidate = extractBalancedJson(text, start);
    if (candidate === null) {
      continue;
    }
    try {
      JSON.parse(candidate);
      return candidate;
    } catch {
      continue;
    }
  }

  const snippet = text.length > 2000 ? `${text.slice(0, 2000)}…` : text;
  throw new Error(`Failed to parse JSON from ${context} output:\n${snippet}`);
}

function extractBalancedJson(text: string, start: number): string | null {
  const open = text[start];
  const close = open === '{' ? '}' : ']';
  let depth = 0;
  let inString = false;
  let escaped = false;

  for (let i = start; i < text.length; i += 1) {
    const ch = text[i];
    if (inString) {
      if (escaped) {
        escaped = false;
      } else if (ch === '\\') {
        escaped = true;
      } else if (ch === '"') {
        inString = false;
      }
      continue;
    }
    if (ch === '"') {
      inString = true;
    } else if (ch === open) {
      depth += 1;
    } else if (ch === close) {
      depth -= 1;
      if (depth === 0) {
        return text.slice(start, i + 1);
      }
    }
  }

  return null;
}

function releaseBinaryPath(): string {
  const binary = path.join(workspaceRoot, 'target', 'release', 'surge');
  return process.platform === 'win32' ? `${binary}.exe` : binary;
}

function readBenchResult(filename: string): BenchResult {
  return JSON.parse(readFileSync(filename, 'utf8'))[0] as BenchResult;
}

type MemoryModeMeta = Pick<
  MemoryModeResult,
  'command' | 'mode' | 'format' | 'compatReport' | 'timings' | 'rustJobs'
>;

function measuredEnv(): NodeJS.ProcessEnv {
  return {
    ...process.env,
    npm_config_cache: process.env.npm_config_cache ?? path.join(os.tmpdir(), 'npm-cache'),
  };
}

function assertMeasuredOk(result: MeasuredCommandResult): void {
  if (result.error) {
    throw new Error(`${result.command} failed: ${result.error}`);
  }
  if (result.status !== 0) {
    throw new Error(
      `${result.command} ${result.args.join(' ')} exited with ${result.status ?? 'unknown'}:\n${result.stdout}${result.stderr}`,
    );
  }
}

function toMemoryModeResult(
  result: MeasuredCommandResult,
  meta: MemoryModeMeta,
): MemoryModeResult {
  return {
    ...meta,
    peakRssBytes: result.peakRssBytes,
    peakRssMb: peakRssMb(result.peakRssBytes),
    peakRssSource: result.peakRssSource,
    durationMs: Math.round(result.durationMs),
    status: result.status,
  };
}

function measureRustMode(
  binary: string,
  tsconfig: string,
  extraArgs: string[],
  meta: MemoryModeMeta,
): MemoryModeResult {
  const result = runMeasuredCommand(binary, ['--project', tsconfig, ...extraArgs], {
    cwd: workspaceRoot,
    env: measuredEnv(),
  });
  assertMeasuredOk(result);
  return toMemoryModeResult(result, meta);
}

/// Best-effort peak-RSS samples for the baseline compilers. Both are invoked
/// through their direct Node entry point: `tsc` runs Node directly, and the
/// `tsgo` shim `execve`s into the native binary in place (same PID), so
/// `/usr/bin/time` measures the compiler process in both cases. Anything that
/// cannot be resolved or run is skipped rather than failing the measurement.
function baselineCompilerMemory(tsconfig: string): MemoryModeResult[] {
  const results: MemoryModeResult[] = [];
  const baseArgs = ['--noEmit', '--pretty', 'false', '--project', tsconfig];

  const tscEntry = path.join(workspaceRoot, 'node_modules', 'typescript', 'bin', 'tsc');
  if (existsSync(tscEntry)) {
    const result = runMeasuredCommand(process.execPath, [tscEntry, ...baseArgs], {
      cwd: workspaceRoot,
      env: measuredEnv(),
    });
    // tsc exits non-zero when it reports diagnostics; that is expected here and
    // does not invalidate the peak-RSS sample.
    results.push(
      toMemoryModeResult(result, {
        command: 'tsc',
        mode: 'noEmit',
        format: 'text',
        compatReport: false,
        timings: false,
        rustJobs: null,
      }),
    );
  }

  const tsgoEntry = path.join(
    workspaceRoot,
    'node_modules',
    '@typescript',
    'native-preview',
    'bin',
    'tsgo.js',
  );
  if (existsSync(tsgoEntry)) {
    const result = runMeasuredCommand(process.execPath, [tsgoEntry, ...baseArgs], {
      cwd: workspaceRoot,
      env: measuredEnv(),
    });
    if (!result.error) {
      results.push(
        toMemoryModeResult(result, {
          command: 'tsgo',
          mode: 'noEmit',
          format: 'text',
          compatReport: false,
          timings: false,
          rustJobs: null,
        }),
      );
    }
  }

  return results;
}

function toMemoryJson(result: MemoryModeResult): Record<string, unknown> {
  return {
    command: result.command,
    mode: result.mode,
    format: result.format,
    compatReport: result.compatReport,
    timings: result.timings,
    rustJobs: result.rustJobs,
    peak_rss_bytes: result.peakRssBytes,
    peak_rss_mb: result.peakRssMb,
    peak_rss_source: result.peakRssSource,
    duration_ms: result.durationMs,
    status: result.status,
  };
}

function formatPeakRss(result: MemoryModeResult): string {
  if (result.peakRssBytes === null || result.peakRssMb === null) {
    return `unavailable (${result.peakRssSource})`;
  }
  if (result.peakRssMb >= 1024) {
    return `${(result.peakRssMb / 1024).toFixed(2)} GB`;
  }
  return `${result.peakRssMb.toFixed(0)} MB`;
}

function formatMemorySection(memoryResults: MemoryModeResult[]): string[] {
  if (memoryResults.length === 0) {
    return ['- none'];
  }
  const lines = ['| Command | Mode | Peak RSS | Wall |', '| --- | --- | ---: | ---: |'];
  for (const result of memoryResults) {
    const wall = `${(result.durationMs / 1000).toFixed(2)}s`;
    lines.push(`| ${result.command} | ${result.mode} | ${formatPeakRss(result)} | ${wall} |`);
  }
  return lines;
}

function renderMeasurementMarkdown(input: {
  name: string;
  project: ResolvedProject;
  timestamp: string;
  commandUsed: string;
  maxDiagnostics: number;
  oracle: OracleComparison;
  compatReport: CompatReport;
  benchResults: Array<{ jobs: RustJobValue; result: BenchResult }>;
  programMeasurements: ProgramMeasurements;
  memoryResults: MemoryModeResult[];
}): string {
  const { oracle, compatReport, programMeasurements } = input;
  const rawOnlyTs = oracle.details?.onlyTypeScript?.rawDiagnosticFingerprints ?? [];
  const rawOnlyRust = oracle.details?.onlySurgeTs?.rawDiagnosticFingerprints ?? [];
  const fileCodeLineMismatch =
    (oracle.matches?.onlyTypeScriptFileCodeLine?.length ?? 0) +
    (oracle.matches?.onlySurgeTsFileCodeLine?.length ?? 0);

  const lines: string[] = [
    `# ${input.name} Measurement`,
    '',
    'Raw real-project compatibility and performance measurement. This report is',
    'measurement only; root-cause analysis is intentionally deferred to an',
    'implementation follow-up.',
    '',
    '## Project',
    `- name: ${input.name}`,
    `- root: \`${input.project.root}\``,
    `- tsconfig: \`${input.project.tsconfig}\``,
    `- timestamp: ${input.timestamp}`,
    `- maxDiagnostics: ${input.maxDiagnostics}`,
    `- command used: \`${input.commandUsed}\``,
    '',
    '## Oracle Comparison',
    `- TypeScript total diagnostics: ${oracle.typescript.total}`,
    `- surge-ts total diagnostics: ${oracle.surgeTs.total}`,
    `- code-count match: ${boolToYesNo(oracle.summary.byCodeMatch)}`,
    `- file/code match: ${boolToYesNo(oracle.summary.byFileCodeMatch)}`,
    `- file/code/line match: ${oracle.summary.byFileCodeLineMatch === null ? 'n/a' : boolToYesNo(oracle.summary.byFileCodeLineMatch)}`,
    `- only-TypeScript diagnostics: ${sumFingerprints(rawOnlyTs)}`,
    `- only-surge-ts diagnostics: ${sumFingerprints(rawOnlyRust)}`,
    '',
    '### Only TypeScript Fingerprints',
    ...formatFingerprintList(rawOnlyTs),
    '',
    '### Only surge-ts Fingerprints',
    ...formatFingerprintList(rawOnlyRust),
    '',
    '## Compat Report',
    `- files loaded total: ${compatReport.filesLoaded}`,
    `- root source files: ${compatReport.loadedSourceFiles}`,
    `- root declaration files: ${compatReport.loadedRootDeclarationFiles}`,
    `- dependency declaration files: ${compatReport.loadedDependencyDeclarationFiles}`,
    `- generated declaration files: ${compatReport.loadedGeneratedDeclarationFiles}`,
    `- dependency declaration diagnostics: ${compatReport.diagnosticsDependencyDeclarationTotal}`,
    `- suppressed rust-only diagnostics: ${compatReport.suppressedRustOnlyDiagnosticsTotal}`,
    '',
    '## Memory',
    'Peak resident set size per command/mode. Raw bytes are in `memory.json`.',
    '',
    ...formatMemorySection(input.memoryResults),
    '',
    '## Timing Buckets',
    ...formatMeasurementMap(programMeasurements.timings),
    '',
    '## Counters',
    ...formatMeasurementMap(programMeasurements.counters),
    '',
    '## Benchmark Medians',
  ];

  for (const { jobs, result } of input.benchResults) {
    const tools = Object.keys(result.stats);
    lines.push(
      '',
      `### rustJobs=${jobs}`,
      '| tool | median | drift |',
      '| --- | ---: | --- |',
      ...tools.map((tool) => {
        const stat = result.stats[tool];
        const drift = result.drift[tool] ?? '';
        return `| ${toolDisplayLabel(tool)} | ${formatSeconds(stat)} | ${drift} |`;
      }),
    );
  }

  lines.push(
    '',
    '## Next-Action Triage (raw buckets only)',
    `- only-TypeScript top codes: ${formatTopCodes(rawOnlyTs)}`,
    `- only-surge-ts top codes: ${formatTopCodes(rawOnlyRust)}`,
    `- file/code/line mismatch buckets: ${fileCodeLineMismatch}`,
    `- crash/error: ${formatWarnings(oracle.warnings)}`,
    '',
    'Root-cause diagnosis is out of scope for this measurement layer. Classify and',
    'diagnose the buckets above in an implementation follow-up, not here.',
    '',
  );

  return lines.join('\n');
}

function parseProgramMeasurements(stderr: string): ProgramMeasurements {
  const timings = new Map<string, string>();
  const counters = new Map<string, string>();
  let section: 'timings' | 'counters' | null = null;

  for (const line of stderr.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (trimmed === 'Timings:') {
      section = 'timings';
      continue;
    }
    if (trimmed === 'counters:') {
      section = 'counters';
      continue;
    }
    if (!section || trimmed.length === 0) {
      continue;
    }

    const match = line.match(/^\s+([A-Za-z0-9_]+):\s+(.*)$/);
    if (!match) {
      continue;
    }

    const [, key, value] = match;
    if (section === 'timings') {
      timings.set(key, value);
    } else {
      counters.set(key, value);
    }
  }

  return { timings, counters };
}

function formatMeasurementMap(map: Map<string, string>): string[] {
  if (map.size === 0) {
    return ['- none'];
  }
  return [...map.entries()].map(([key, value]) => `- ${key}: ${value}`);
}

function sumFingerprints(fingerprints: DiagnosticFingerprint[]): number {
  return fingerprints.reduce((total, entry) => total + entry.count, 0);
}

function formatTopCodes(fingerprints: DiagnosticFingerprint[]): string {
  if (fingerprints.length === 0) {
    return 'none';
  }
  const byCode = new Map<string, number>();
  for (const entry of fingerprints) {
    byCode.set(entry.code, (byCode.get(entry.code) ?? 0) + entry.count);
  }
  return [...byCode.entries()]
    .sort((a, b) => b[1] - a[1])
    .slice(0, 10)
    .map(([code, count]) => `${code}=${count}`)
    .join(', ');
}

function formatWarnings(warnings: string[] | undefined): string {
  return warnings && warnings.length > 0 ? warnings.join('; ') : 'none';
}

function formatFingerprintList(fingerprints: DiagnosticFingerprint[]): string[] {
  if (fingerprints.length === 0) {
    return ['- none'];
  }

  return fingerprints.slice(0, 10).map((entry) => {
    const location = `${entry.fileName}:${entry.line ?? 'n/a'}:${entry.column ?? 'n/a'}`;
    const message = entry.message ?? '(no message)';
    return `- ${location} ${entry.code} ${entry.count} ${message}`;
  });
}

function formatSeconds(stats: BenchStats | null | undefined): string {
  return stats ? `${stats.median.toFixed(2)}s` : 'n/a';
}

function boolToYesNo(value: boolean): string {
  return value ? 'yes' : 'no';
}

const invokedDirectly =
  Boolean(process.argv[1]) && path.resolve(process.argv[1]) === scriptPath;
if (invokedDirectly) {
  main();
}
