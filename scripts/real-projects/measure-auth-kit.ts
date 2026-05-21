#!/usr/bin/env tsx

import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

type OracleComparison = {
  project: string | null;
  typescript: {
    total: number;
    byCode: Array<{ key: string; count: number }>;
  };
  typescriptRust: {
    total: number;
  };
  summary: {
    byCodeMatch: boolean;
    byFileCodeMatch: boolean;
    byFileCodeLineMatch: boolean | null;
  };
  details?: {
    onlyTypeScript?: {
      rawDiagnosticFingerprints?: Array<{
        fileName: string;
        code: string;
        line: number | null;
        column: number | null;
        message: string | null;
        count: number;
      }>;
    };
    onlyTypeScriptRust?: {
      rawDiagnosticFingerprints?: Array<{
        fileName: string;
        code: string;
        line: number | null;
        column: number | null;
        message: string | null;
        count: number;
      }>;
    };
  };
};

type CompatReport = {
  filesLoaded: number;
  loadedSourceFiles: number;
  loadedRootDeclarationFiles: number;
  loadedDependencyDeclarationFiles: number;
  loadedGeneratedDeclarationFiles: number;
  diagnosticsDependencyDeclarationTotal: number;
  suppressedRustOnlyDiagnosticsTotal: number;
};

type BenchStats = {
  median: number;
  min: number;
  max: number;
  runs: number;
};

type BenchResult = {
  project: string;
  rustJobs?: number;
  stats: Record<string, BenchStats | null>;
  drift: Record<string, string>;
};

type ResolvedProject = {
  root: string;
  tsconfig: string;
  attempted: string[];
};

const scriptPath = fileURLToPath(import.meta.url);
const scriptDir = path.dirname(scriptPath);
const workspaceRoot = path.resolve(scriptDir, '../..');
const benchDir = path.join(workspaceRoot, '.bench');
const candidateRoots = [
  process.env.AUTH_KIT_PROJECT,
  path.resolve(workspaceRoot, '../../typescript/auth-project/auth-kit'),
  path.resolve(workspaceRoot, '.local-projects/auth-kit'),
].filter((value): value is string => Boolean(value));

function main(argv = process.argv.slice(2)): void {
  const allowMissing = argv.includes('--allowMissing');
  mkdirSync(benchDir, { recursive: true });

  const project = resolveAuthKitProject();
  if (!project) {
    const message = [
      'auth-kit unavailable',
      'Attempted paths:',
      ...candidateRoots.map((candidate) => `  - ${candidate}`),
    ].join('\n');
    process.stdout.write(`${message}\n`);
    writeFileSync(path.join(benchDir, 'auth-kit-measurement.md'), `${message}\n`);
    if (!allowMissing) {
      process.exitCode = 1;
      return;
    }
    process.exitCode = 0;
    return;
  }

  const oracleText = runCommand('pnpm', [
    'run',
    'oracle:compare',
    '--',
    '--project',
    project.tsconfig,
    '--maxDiagnostics',
    '500',
  ]);
  writeFileSync(path.join(benchDir, 'auth-kit-oracle-compare.txt'), oracleText);

  const oracleJsonText = runCommand('pnpm', [
    'run',
    'oracle:compare',
    '--',
    '--project',
    project.tsconfig,
    '--maxDiagnostics',
    '500',
    '--json',
  ]);
  const oracle = JSON.parse(oracleJsonText) as OracleComparison;

  runCommand('cargo', ['build', '--release', '-p', 'typescript-rust-cli']);

  const compatReportText = runCommand(releaseBinaryPath(), [
    '--project',
    project.tsconfig,
    '--compatReport',
    '--format',
    'json',
  ]);
  const compatReport = JSON.parse(compatReportText) as CompatReport;

  runCommand('pnpm', [
    'exec',
    'tsx',
    path.join(scriptDir, '../bench/compare-compilers.ts'),
    '--',
    '--project',
    project.tsconfig,
    '--json',
    path.join(benchDir, 'auth-kit-jobs1.json'),
    '--html',
    path.join(benchDir, 'auth-kit-jobs1.html'),
    '--chart',
    path.join(benchDir, 'auth-kit-jobs1.svg'),
    '--rustJobs',
    '1',
  ]);

  runCommand('pnpm', [
    'exec',
    'tsx',
    path.join(scriptDir, '../bench/compare-compilers.ts'),
    '--',
    '--project',
    project.tsconfig,
    '--json',
    path.join(benchDir, 'auth-kit-jobs4.json'),
    '--html',
    path.join(benchDir, 'auth-kit-jobs4.html'),
    '--chart',
    path.join(benchDir, 'auth-kit-jobs4.svg'),
    '--rustJobs',
    '4',
  ]);

  const jobs1 = readBenchResult(path.join(benchDir, 'auth-kit-jobs1.json'));
  const jobs4 = readBenchResult(path.join(benchDir, 'auth-kit-jobs4.json'));
  const markdown = renderMeasurementMarkdown(project, oracle, compatReport, jobs1, jobs4);
  writeFileSync(path.join(benchDir, 'auth-kit-measurement.md'), markdown);
}

function resolveAuthKitProject(): ResolvedProject | null {
  const attempted: string[] = [];

  for (const candidate of candidateRoots) {
    const root = path.isAbsolute(candidate) ? candidate : path.resolve(workspaceRoot, candidate);
    attempted.push(root);

    const tsconfig = path.join(root, 'tsconfig.json');
    if (existsSync(tsconfig)) {
      return { root, tsconfig, attempted };
    }

    if (path.basename(root).toLowerCase() === 'tsconfig.json' && existsSync(root)) {
      return { root: path.dirname(root), tsconfig: root, attempted };
    }
  }

  return null;
}

function runCommand(command: string, args: string[]): string {
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
      `${command} exited with ${result.status ?? 'unknown'}:\n${result.stdout ?? ''}${result.stderr ?? ''}`,
    );
  }

  return `${result.stdout ?? ''}`;
}

function releaseBinaryPath(): string {
  const binary = path.join(workspaceRoot, 'target', 'release', 'typescript-rust-cli');
  return process.platform === 'win32' ? `${binary}.exe` : binary;
}

function readBenchResult(filename: string): BenchResult {
  return JSON.parse(readFileSync(filename, 'utf8'))[0] as BenchResult;
}

function renderMeasurementMarkdown(
  project: ResolvedProject,
  oracle: OracleComparison,
  compatReport: CompatReport,
  jobs1: BenchResult,
  jobs4: BenchResult,
): string {
  const counts = new Map(oracle.typescript.byCode.map((entry) => [entry.key, entry.count]));
  const rawOnlyRust = oracle.details?.onlyTypeScriptRust?.rawDiagnosticFingerprints ?? [];
  const rawOnlyTs = oracle.details?.onlyTypeScript?.rawDiagnosticFingerprints ?? [];
  const repeatedFingerprints = [...rawOnlyTs, ...rawOnlyRust].filter((entry) => entry.count > 1);

  return [
    '# Auth-Kit Measurement',
    '',
    `Project path used: \`${project.root}\``,
    `tsconfig.json: \`${project.tsconfig}\``,
    '',
    '## Raw Totals',
    `- TypeScript total diagnostics: ${oracle.typescript.total}`,
    `- typescript-rust total diagnostics: ${oracle.typescriptRust.total}`,
    `- code-count match: ${boolToYesNo(oracle.summary.byCodeMatch)}`,
    `- file/code match: ${boolToYesNo(oracle.summary.byFileCodeMatch)}`,
    `- file/code/line match: ${oracle.summary.byFileCodeLineMatch === null ? 'n/a' : boolToYesNo(oracle.summary.byFileCodeLineMatch)}`,
    '',
    '## Compat Report Totals',
    `- files loaded total: ${compatReport.filesLoaded}`,
    `- root source files: ${compatReport.loadedSourceFiles}`,
    `- root declarations: ${compatReport.loadedRootDeclarationFiles}`,
    `- dependency declarations: ${compatReport.loadedDependencyDeclarationFiles}`,
    `- generated files: ${compatReport.loadedGeneratedDeclarationFiles}`,
    `- diagnostics from dependency declarations: ${compatReport.diagnosticsDependencyDeclarationTotal}`,
    `- Rust-only typescript-rust::* diagnostics in tsc profile: ${compatReport.suppressedRustOnlyDiagnosticsTotal}`,
    '',
    '## TypeScript Code Counts',
    ...['TS2304', 'TS2305', 'TS2307', 'TS2339', 'TS2353', 'TS2322', 'TS2367'].map(
      (code) => `- ${code}: ${counts.get(code) ?? 0}`,
    ),
    '',
    '## Top ONLY_RUST Raw Diagnostic Fingerprints',
    ...formatFingerprintList(rawOnlyRust),
    '',
    '## Top ONLY_TS Raw Diagnostic Fingerprints',
    ...formatFingerprintList(rawOnlyTs),
    '',
    '## Repeated Raw Diagnostic Fingerprints',
    ...(repeatedFingerprints.length > 0
      ? formatFingerprintList(repeatedFingerprints)
      : ['- none']),
    '',
    '## Benchmark Medians',
    '| tool | jobs=1 | jobs=4 |',
    '| --- | ---: | ---: |',
    ...(['tsc', 'tsgo', 'tsgo-singleThreaded', 'ts-rust'] as const).map((tool) => {
      const stat1 = jobs1.stats[tool];
      const stat4 = jobs4.stats[tool];
      return `| ${tool} | ${formatSeconds(stat1)} | ${formatSeconds(stat4)} |`;
    }),
    '',
  ].join('\n');
}

function formatFingerprintList(
  fingerprints: Array<{
    fileName: string;
    code: string;
    line: number | null;
    column: number | null;
    message: string | null;
    count: number;
  }>,
): string[] {
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

main();
