#!/usr/bin/env tsx

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, statSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export type Source = 'typescript' | 'typescript-rust';

export type NormalizedDiagnostic = {
  source: Source;
  code: string;
  fileName: string;
  line?: number;
  column?: number;
  message?: string;
};

export type CountBucket = {
  key: string;
  typescript: number;
  typescriptRust: number;
};

export type CountEntry = {
  key: string;
  count: number;
};

export type DiagnosticTotals = {
  total: number;
  byCode: CountEntry[];
  byFileCode: CountEntry[];
  byFileCodeLine: CountEntry[];
};

export type ComparisonResult = {
  mode: 'project' | 'file';
  project: string | null;
  file: string | null;
  ignoreConfig?: boolean;
  typescriptRustOptions?: {
    stubExternalModules?: boolean;
  };
  tooling: {
    typescriptVersion: string;
    typescriptCommand: string;
    typescriptRustCommand: string;
  };
  typescript: DiagnosticTotals;
  typescriptRust: DiagnosticTotals;
  matches: {
    byCode: CountBucket[];
    onlyTypeScript: CountBucket[];
    onlyTypeScriptRust: CountBucket[];
    byFileCode: CountBucket[];
    onlyTypeScriptFileCode: CountBucket[];
    onlyTypeScriptRustFileCode: CountBucket[];
    byFileCodeLine: CountBucket[];
    onlyTypeScriptFileCodeLine: CountBucket[];
    onlyTypeScriptRustFileCodeLine: CountBucket[];
  };
  summary: {
    byCodeMatch: boolean;
    byFileCodeMatch: boolean;
    byFileCodeLineMatch: boolean | null;
  };
};

export type ParsedArgs = {
  projectInput?: string;
  fileInput?: string;
  json: boolean;
  failOnMismatch: boolean;
  maxDiagnostics?: number;
  ignoreConfig?: boolean;
  stubExternalModules?: boolean;
};

export type OracleMode =
  | {
      kind: 'project';
      project: string;
      resolvedTsconfig: string;
      ignoreConfig?: boolean;
      stubExternalModules?: boolean;
    }
  | {
      kind: 'file';
      file: string;
      resolvedFile: string;
      ignoreConfig?: boolean;
      stubExternalModules?: boolean;
    };

export type RunResult = {
  exitCode: number | null;
  stdout: string;
  stderr: string;
};

const scriptPath = fileURLToPath(import.meta.url);
const scriptDir = path.dirname(scriptPath);
const workspaceRoot = path.resolve(scriptDir, '../..');
const packageManagerCache = process.env.npm_config_cache ?? path.join(os.tmpdir(), 'npm-cache');
const packageManagerExecutable = process.env.npm_execpath ? process.execPath : 'pnpm';
const packageManagerArgsPrefix = process.env.npm_execpath ? [process.env.npm_execpath] : [];
const pinnedTypeScriptVersion = readPinnedTypeScriptVersion();
const subprocessMaxBuffer = 50 * 1024 * 1024;

const fixturePresets: Record<string, string> = {
  'declarations-basic': path.join(workspaceRoot, 'tests/compat-projects/declarations-basic/tsconfig.json'),
  'declarations-hardening': path.join(workspaceRoot, 'tests/compat-projects/declarations-hardening/tsconfig.json'),
  'diagnostics-pack': path.join(workspaceRoot, 'tests/compat-projects/diagnostics-pack/tsconfig.json'),
  'generics-basic': path.join(workspaceRoot, 'tests/compat-projects/generics-basic/tsconfig.json'),
  'package-imports': path.join(workspaceRoot, 'tests/compat-projects/package-imports/tsconfig.json'),
  'module-forms': path.join(workspaceRoot, 'tests/compat-projects/module-forms/tsconfig.json'),
  'relative-deep': path.join(workspaceRoot, 'tests/compat-projects/relative-deep/tsconfig.json'),
  'private-types': path.join(workspaceRoot, 'tests/compat-projects/private-types/tsconfig.json'),
  'package-declarations': path.join(workspaceRoot, 'tests/compat-projects/package-declarations/tsconfig.json'),
};

export function main(argv = process.argv.slice(2)): void {
  const args = parseArgs(argv);
  const mode = resolveOracleMode(args);
  const comparison =
    mode.kind === 'project'
      ? compareProject(mode.resolvedTsconfig, displayComparisonTargetPath(mode.resolvedTsconfig), args.maxDiagnostics, mode.stubExternalModules)
      : compareFile(mode.resolvedFile, displayComparisonTargetPath(mode.resolvedFile), args.maxDiagnostics, mode.ignoreConfig, mode.stubExternalModules);

  if (args.json) {
    process.stdout.write(`${JSON.stringify(comparison, null, 2)}\n`);
  } else {
    process.stdout.write(renderComparisonText(comparison));
  }

  const hasMismatch = !comparison.summary.byCodeMatch || !comparison.summary.byFileCodeMatch;
  if (args.failOnMismatch && hasMismatch) {
    process.exitCode = 1;
  }
}

export function parseArgs(argv: string[]): ParsedArgs {
  const parsed: ParsedArgs = {
    json: false,
    failOnMismatch: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === '--help' || arg === '-h') {
      printHelpAndExit();
    } else if (arg === '--') {
      continue;
    } else if (arg === '--project' || arg === '--fixture') {
      const value = argv[++index];
      if (!value) {
        throw new Error(`${arg} requires a value`);
      }
      parsed.projectInput = value;
    } else if (arg === '--file') {
      const value = argv[++index];
      if (!value) {
        throw new Error('--file requires a value');
      }
      parsed.fileInput = value;
    } else if (arg === '--json') {
      parsed.json = true;
    } else if (arg === '--ignoreConfig') {
      parsed.ignoreConfig = true;
    } else if (arg === '--stubExternalModules') {
      parsed.stubExternalModules = true;
    } else if (arg === '--failOnMismatch' || arg === '--strictCodes') {
      parsed.failOnMismatch = true;
    } else if (arg === '--maxDiagnostics') {
      const value = argv[++index];
      if (!value) {
        throw new Error('--maxDiagnostics requires a value');
      }
      const parsedValue = Number(value);
      if (!Number.isInteger(parsedValue) || parsedValue <= 0) {
        throw new Error('--maxDiagnostics must be greater than 0');
      }
      parsed.maxDiagnostics = parsedValue;
    } else if (arg.startsWith('--')) {
      throw new Error(`unknown argument: ${arg}`);
    } else {
      throw new Error(`unexpected positional argument: ${arg}. Use --project <path-or-preset> or --file <path>.`);
    }
  }

  return parsed;
}

export function resolveOracleMode(args: ParsedArgs): OracleMode {
  const hasProject = args.projectInput !== undefined;
  const hasFile = args.fileInput !== undefined;

  if (hasProject === hasFile) {
    throw new Error('choose exactly one of --project or --file.');
  }

  if (hasProject) {
    if (args.ignoreConfig) {
      console.error('error: --ignoreConfig is only supported with --file in the oracle.');
      process.exit(1);
    }
    const projectInput = args.projectInput as string;
    const resolvedTsconfig = resolveProjectPresetOrPath(projectInput);
    return {
      kind: 'project',
      project: projectInput,
      resolvedTsconfig,
      stubExternalModules: args.stubExternalModules,
    };
  }

  const fileInput = args.fileInput as string;
  const resolvedFile = resolveFilePath(fileInput);
  return {
    kind: 'file',
    file: fileInput,
    resolvedFile,
    ignoreConfig: args.ignoreConfig,
    stubExternalModules: args.stubExternalModules,
  };
}

export function resolveProjectPresetOrPath(projectInput: string): string {
  const preset = fixturePresets[projectInput];
  if (preset) {
    return preset;
  }

  if (isSourceFilePath(projectInput)) {
    throw new Error(
      `--project expects a preset name or tsconfig.json path. For single files, use --file ${projectInput}.`,
    );
  }

  if (isTsConfigPath(projectInput)) {
    const tsconfigPath = resolveWorkspacePath(projectInput);
    if (!existsSync(tsconfigPath) || !statSync(tsconfigPath).isFile()) {
      throw new Error(`missing tsconfig.json at ${normalizePathForDisplay(tsconfigPath)}`);
    }

    return tsconfigPath;
  }

  if (looksLikePath(projectInput)) {
    const candidate = resolveWorkspacePath(projectInput);
    if (existsSync(candidate)) {
      const stats = statSync(candidate);
      if (stats.isDirectory()) {
        const tsconfigPath = path.join(candidate, 'tsconfig.json');
        if (existsSync(tsconfigPath) && statSync(tsconfigPath).isFile()) {
          return tsconfigPath;
        }

        throw new Error(`missing tsconfig.json at ${normalizePathForDisplay(tsconfigPath)}`);
      }

      if (stats.isFile() && isTsConfigPath(candidate)) {
        return candidate;
      }
    }

    if (projectInput.endsWith('.json')) {
      if (path.basename(projectInput).toLowerCase().includes('tsconfig')) {
        throw new Error(`missing tsconfig.json at ${normalizePathForDisplay(candidate)}`);
      }

      throw new Error(
        `--project expects a preset name or tsconfig.json path. For single files, use --file ${projectInput}.`,
      );
    }

    const tsconfigPath = path.join(candidate, 'tsconfig.json');
    throw new Error(`missing tsconfig.json at ${normalizePathForDisplay(tsconfigPath)}`);
  }

  throw new Error(`unknown oracle project preset: ${projectInput}`);
}

export function resolveFilePath(fileInput: string): string {
  if (isTsConfigPath(fileInput)) {
    throw new Error('--file expects a TypeScript source file, not tsconfig.json. For projects, use --project.');
  }

  if (fileInput.toLowerCase().endsWith('.d.ts')) {
    throw new Error(`--file currently supports .ts source files only. Received ${fileInput}.`);
  }

  const extension = path.extname(fileInput).toLowerCase();
  if (extension !== '.ts') {
    if (isSourceFilePath(fileInput)) {
      throw new Error(`--file currently supports .ts source files only. Received ${fileInput}.`);
    }

    throw new Error(`--file currently supports .ts source files only. Received ${fileInput}.`);
  }

  const resolvedFile = resolveWorkspacePath(fileInput);
  if (!existsSync(resolvedFile) || !statSync(resolvedFile).isFile()) {
    throw new Error(`missing TypeScript source file: ${normalizePathForDisplay(resolvedFile)}`);
  }

  return resolvedFile;
}

export function resolveProjectInput(projectInput: string): string {
  return resolveProjectPresetOrPath(projectInput);
}

export function compareProject(
  tsconfigPath: string,
  projectDisplay: string,
  maxDiagnostics?: number,
  stubExternalModules?: boolean,
): ComparisonResult {
  return executeComparison(
    {
      kind: 'project',
      project: projectDisplay,
      resolvedTsconfig: tsconfigPath,
      stubExternalModules,
    },
    maxDiagnostics,
  );
}

export function compareFile(
  filePath: string,
  fileDisplay: string,
  maxDiagnostics?: number,
  ignoreConfig?: boolean,
  stubExternalModules?: boolean,
): ComparisonResult {
  return executeComparison(
    {
      kind: 'file',
      file: fileDisplay,
      resolvedFile: filePath,
      ignoreConfig,
      stubExternalModules,
    },
    maxDiagnostics,
  );
}

export function runTsc(mode: OracleMode): RunResult {
  const args =
    mode.kind === 'project'
      ? ['exec', 'tsc', '--noEmit', '--pretty', 'false', '--project', mode.resolvedTsconfig]
      : ['exec', 'tsc', '--noEmit', '--pretty', 'false', mode.resolvedFile];
  if (mode.ignoreConfig) {
      args.splice(args.length - 1, 0, '--ignoreConfig');
  }
  const result = spawnSync(packageManagerExecutable, [...packageManagerArgsPrefix, ...args], {
    cwd: workspaceRoot,
    encoding: 'utf8',
    maxBuffer: subprocessMaxBuffer,
    env: {
      ...process.env,
      npm_config_cache: packageManagerCache,
    },
  });

  if (result.error) {
    throw new Error(`failed to run TypeScript compiler: ${result.error.message}`);
  }

  return {
    exitCode: result.status,
    stdout: result.stdout ?? '',
    stderr: result.stderr ?? '',
  };
}

export function runTypeScriptRust(mode: OracleMode, maxDiagnostics?: number): RunResult {
  const args = [
    'run',
    '-q',
    '--manifest-path',
    path.join(workspaceRoot, 'Cargo.toml'),
    '-p',
    'typescript-rust-cli',
    '--',
  ];

  if (mode.kind === 'project') {
    args.push('--project', mode.resolvedTsconfig);
    args.push('--format', 'json');
    if (mode.stubExternalModules) {
      args.push('--stubExternalModules');
    }
  } else {
    args.push('--format', 'json');
    if (mode.ignoreConfig) {
      args.push('--ignoreConfig');
    }
    if (mode.stubExternalModules) {
      args.push('--stubExternalModules');
    }
    if (maxDiagnostics !== undefined) {
      args.push('--maxDiagnostics', String(maxDiagnostics));
    }
    args.push(mode.resolvedFile);
    const result = spawnSync('cargo', args, {
      cwd: workspaceRoot,
      encoding: 'utf8',
      maxBuffer: subprocessMaxBuffer,
    });

    if (result.error) {
      throw new Error(`failed to run typescript-rust-cli: ${result.error.message}`);
    }

    return {
      exitCode: result.status,
      stdout: result.stdout ?? '',
      stderr: result.stderr ?? '',
    };
  }

  if (maxDiagnostics !== undefined) {
    args.push('--maxDiagnostics', String(maxDiagnostics));
  }

  const result = spawnSync('cargo', args, {
    cwd: workspaceRoot,
    encoding: 'utf8',
    maxBuffer: subprocessMaxBuffer,
  });

  if (result.error) {
    throw new Error(`failed to run typescript-rust-cli: ${result.error.message}`);
  }

  return {
    exitCode: result.status,
    stdout: result.stdout ?? '',
    stderr: result.stderr ?? '',
  };
}

export function parseTypeScriptDiagnostics(output: string, projectDir: string): NormalizedDiagnostic[] {
  const diagnostics: NormalizedDiagnostic[] = [];
  const lines = output.split(/\r?\n/);

  for (const rawLine of lines) {
    const line = rawLine.trimEnd();
    if (!line) {
      continue;
    }

    const fileDiagnostic = line.match(/^(.*)\((\d+),(\d+)\): error (TS\d+): (.*)$/);
    if (fileDiagnostic) {
      diagnostics.push({
        source: 'typescript',
        fileName: normalizeDiagnosticFileName(projectDir, fileDiagnostic[1]),
        line: Number(fileDiagnostic[2]),
        column: Number(fileDiagnostic[3]),
        code: fileDiagnostic[4],
        message: fileDiagnostic[5],
      });
      continue;
    }

    const globalDiagnostic = line.match(/^error (TS\d+): (.*)$/);
    if (globalDiagnostic) {
      diagnostics.push({
        source: 'typescript',
        fileName: '',
        code: globalDiagnostic[1],
        message: globalDiagnostic[2],
      });
    }
  }

  return diagnostics;
}

export function parseTypeScriptRustDiagnostics(
  output: string,
  projectDir: string,
): NormalizedDiagnostic[] {
  let parsed: unknown;
  try {
    parsed = JSON.parse(output);
  } catch (error) {
    throw new Error(
      `typescript-rust-cli did not emit valid JSON diagnostics.\n${formatParseFailure(output, error)}`,
    );
  }

  const diagnostics = Array.isArray((parsed as { diagnostics?: unknown }).diagnostics)
    ? ((parsed as { diagnostics: unknown[] }).diagnostics ?? [])
    : [];

  return diagnostics.map((diagnostic) => {
    const entry = diagnostic as {
      code?: unknown;
      fileName?: unknown;
      line?: unknown;
      column?: unknown;
      message?: unknown;
    };

    return {
      source: 'typescript-rust',
      code: String(entry.code ?? ''),
      fileName: normalizeDiagnosticFileName(projectDir, String(entry.fileName ?? '')),
      line: typeof entry.line === 'number' ? entry.line : undefined,
      column: typeof entry.column === 'number' ? entry.column : undefined,
      message: typeof entry.message === 'string' ? entry.message : undefined,
    };
  });
}

export function normalizeDiagnosticFileName(projectDir: string, fileName: string): string {
  if (!fileName) {
    return '';
  }

  const normalizedWorkspaceRoot = normalizePathForDisplay(workspaceRoot).replace(/\/+$/, '');
  const normalizedProjectDir = isAbsolutePathLike(projectDir)
    ? normalizePathForDisplay(projectDir).replace(/\/+$/, '')
    : normalizePathForDisplay(path.resolve(projectDir)).replace(/\/+$/, '');
  const workspaceRelativeProjectDir = normalizePathForDisplay(
    path.relative(workspaceRoot, isAbsolutePathLike(projectDir) ? projectDir : path.resolve(projectDir)),
  ).replace(/\/+$/, '');
  const normalizedInputFileName = normalizePathForDisplay(fileName);

  if (
    workspaceRelativeProjectDir &&
    normalizedInputFileName.startsWith(`${workspaceRelativeProjectDir}/`)
  ) {
    return normalizedInputFileName.slice(workspaceRelativeProjectDir.length + 1);
  }

  if (workspaceRelativeProjectDir && normalizedInputFileName === workspaceRelativeProjectDir) {
    return path.basename(normalizedInputFileName);
  }

  let normalizedFileName = isAbsolutePathLike(fileName)
    ? normalizedInputFileName
    : normalizePathForDisplay(`${normalizedProjectDir}/${normalizedInputFileName}`);

  if (normalizedFileName.startsWith(`${normalizedWorkspaceRoot}/`)) {
    normalizedFileName = normalizedFileName.slice(normalizedWorkspaceRoot.length + 1);
  }

  if (workspaceRelativeProjectDir && normalizedFileName === workspaceRelativeProjectDir) {
    return path.basename(normalizedFileName);
  }

  if (workspaceRelativeProjectDir && normalizedFileName.startsWith(`${workspaceRelativeProjectDir}/`)) {
    return normalizedFileName.slice(workspaceRelativeProjectDir.length + 1);
  }

  if (normalizedFileName === normalizedProjectDir) {
    return path.basename(normalizedFileName);
  }

  const projectPrefix = `${normalizedProjectDir}/`;
  if (normalizedProjectDir && normalizedFileName.startsWith(projectPrefix)) {
    return normalizedFileName.slice(projectPrefix.length);
  }

  return normalizedFileName;
}

export function normalizePathForDisplay(value: string): string {
  return value.replace(/\\/g, '/');
}

export function limitDiagnostics(
  diagnostics: NormalizedDiagnostic[],
  maxDiagnostics?: number,
): NormalizedDiagnostic[] {
  if (maxDiagnostics === undefined) {
    return diagnostics;
  }

  return diagnostics.slice(0, maxDiagnostics);
}

export function compareDiagnostics(
  mode: 'project' | 'file',
  targetDisplay: string,
  typescript: NormalizedDiagnostic[],
  typescriptRust: NormalizedDiagnostic[],
  ignoreConfig?: boolean,
  stubExternalModules?: boolean,
): ComparisonResult {
  const byCode = compareBuckets(typescript, typescriptRust, keyByCode);
  const byFileCode = compareBuckets(typescript, typescriptRust, keyByFileCode);
  const byFileCodeLine = compareBuckets(
    typescript.filter(hasLineInfo),
    typescriptRust.filter(hasLineInfo),
    keyByFileCodeLine,
  );

  return {
    mode,
    project: mode === 'project' ? targetDisplay : null,
    file: mode === 'file' ? targetDisplay : null,
    ignoreConfig: ignoreConfig ?? false,
    typescriptRustOptions: {
      stubExternalModules: stubExternalModules ?? false,
    },
    tooling: {
      typescriptVersion: pinnedTypeScriptVersion,
      typescriptCommand: buildTypeScriptCommand(mode, targetDisplay, ignoreConfig),
      typescriptRustCommand: buildTypeScriptRustCommand(mode, targetDisplay, ignoreConfig, stubExternalModules),
    },
    typescript: summarizeDiagnostics(typescript),
    typescriptRust: summarizeDiagnostics(typescriptRust),
    matches: {
      byCode: byCode.matches,
      onlyTypeScript: byCode.onlyTypeScript,
      onlyTypeScriptRust: byCode.onlyTypeScriptRust,
      byFileCode: byFileCode.matches,
      onlyTypeScriptFileCode: byFileCode.onlyTypeScript,
      onlyTypeScriptRustFileCode: byFileCode.onlyTypeScriptRust,
      byFileCodeLine: byFileCodeLine.matches,
      onlyTypeScriptFileCodeLine: byFileCodeLine.onlyTypeScript,
      onlyTypeScriptRustFileCodeLine: byFileCodeLine.onlyTypeScriptRust,
    },
    summary: {
      byCodeMatch: byCode.onlyTypeScript.length === 0 && byCode.onlyTypeScriptRust.length === 0,
      byFileCodeMatch:
        byFileCode.onlyTypeScript.length === 0 && byFileCode.onlyTypeScriptRust.length === 0,
      byFileCodeLineMatch:
        byFileCodeLine.matches.length > 0 ||
        byFileCodeLine.onlyTypeScript.length > 0 ||
        byFileCodeLine.onlyTypeScriptRust.length > 0
          ? byFileCodeLine.onlyTypeScript.length === 0 &&
            byFileCodeLine.onlyTypeScriptRust.length === 0
          : null,
    },
  };
}

export function buildTypeScriptCommand(mode: 'project' | 'file', targetDisplay: string, ignoreConfig?: boolean): string {
  if (mode === 'project') {
    return `pnpm exec tsc --noEmit --pretty false --project ${targetDisplay}`;
  }

  return ignoreConfig ? `pnpm exec tsc --noEmit --pretty false --ignoreConfig ${targetDisplay}` : `pnpm exec tsc --noEmit --pretty false ${targetDisplay}`;
}

export function buildTypeScriptRustCommand(mode: 'project' | 'file', targetDisplay: string, ignoreConfig?: boolean, stubExternalModules?: boolean): string {
  const cargoToml = normalizePathForDisplay(path.join(workspaceRoot, 'Cargo.toml'));
  let args = `cargo run -q --manifest-path ${cargoToml} -p typescript-rust-cli --`;

  if (mode === 'project') {
    args += ` --project ${targetDisplay} --format json`;
    if (stubExternalModules) {
      args += ` --stubExternalModules`;
    }
    return args;
  }

  args += ` --format json`;
  if (ignoreConfig) {
    args += ` --ignoreConfig`;
  }
  if (stubExternalModules) {
    args += ` --stubExternalModules`;
  }
  args += ` ${targetDisplay}`;

  return args;
}

export function summarizeDiagnostics(diagnostics: NormalizedDiagnostic[]): DiagnosticTotals {
  return {
    total: diagnostics.length,
    byCode: countEntriesFromCounts(countDiagnostics(diagnostics, keyByCode)),
    byFileCode: countEntriesFromCounts(countDiagnostics(diagnostics, keyByFileCode)),
    byFileCodeLine: countEntriesFromCounts(
      countDiagnostics(diagnostics.filter(hasLineInfo), keyByFileCodeLine),
    ),
  };
}

export function compareBuckets(
  left: NormalizedDiagnostic[],
  right: NormalizedDiagnostic[],
  keyFn: (diagnostic: NormalizedDiagnostic) => string,
): {
  matches: CountBucket[];
  onlyTypeScript: CountBucket[];
  onlyTypeScriptRust: CountBucket[];
} {
  const leftCounts = countDiagnostics(left, keyFn);
  const rightCounts = countDiagnostics(right, keyFn);
  const keys = new Set([...leftCounts.keys(), ...rightCounts.keys()]);
  const sortedKeys = [...keys].sort((leftKey, rightKey) => leftKey.localeCompare(rightKey));
  const matches: CountBucket[] = [];
  const onlyTypeScript: CountBucket[] = [];
  const onlyTypeScriptRust: CountBucket[] = [];

  for (const key of sortedKeys) {
    const leftCount = leftCounts.get(key) ?? 0;
    const rightCount = rightCounts.get(key) ?? 0;
    if (leftCount === rightCount) {
      if (leftCount > 0) {
        matches.push({ key, typescript: leftCount, typescriptRust: rightCount });
      }
      continue;
    }

    if (leftCount > 0 && rightCount === 0) {
      onlyTypeScript.push({ key, typescript: leftCount, typescriptRust: 0 });
      continue;
    }

    if (rightCount > 0 && leftCount === 0) {
      onlyTypeScriptRust.push({ key, typescript: 0, typescriptRust: rightCount });
      continue;
    }

    if (leftCount > rightCount) {
      onlyTypeScript.push({ key, typescript: leftCount, typescriptRust: rightCount });
    } else {
      onlyTypeScriptRust.push({ key, typescript: leftCount, typescriptRust: rightCount });
    }
  }

  return { matches, onlyTypeScript, onlyTypeScriptRust };
}

export function countDiagnostics(
  diagnostics: NormalizedDiagnostic[],
  keyFn: (diagnostic: NormalizedDiagnostic) => string,
): Map<string, number> {
  const counts = new Map<string, number>();

  for (const diagnostic of diagnostics) {
    const key = keyFn(diagnostic);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }

  return counts;
}

export function countEntriesFromCounts(counts: Map<string, number>): CountEntry[] {
  return [...counts.entries()]
    .map(([key, count]) => ({ key, count }))
    .sort((left, right) => left.key.localeCompare(right.key));
}

export function keyByCode(diagnostic: NormalizedDiagnostic): string {
  return diagnostic.code;
}

export function keyByFileCode(diagnostic: NormalizedDiagnostic): string {
  return `${diagnostic.fileName} :: ${diagnostic.code}`;
}

export function keyByFileCodeLine(diagnostic: NormalizedDiagnostic): string {
  return `${diagnostic.fileName} :: ${diagnostic.code} :: line=${diagnostic.line ?? 0}`;
}

export function hasLineInfo(diagnostic: NormalizedDiagnostic): boolean {
  return typeof diagnostic.line === 'number' && typeof diagnostic.column === 'number';
}

export function renderComparisonText(comparison: ComparisonResult): string {
  const lines: string[] = [];
  lines.push('TypeScript oracle comparison');
  lines.push(`Mode: ${comparison.mode}`);
  lines.push(comparison.mode === 'project' ? `Project: ${comparison.project}` : `File: ${comparison.file}`);
  lines.push('');

  if (comparison.typescriptRustOptions?.stubExternalModules) {
    lines.push('typescript-rust options: --stubExternalModules');
    lines.push('Note: --stubExternalModules is a typescript-rust-only compatibility mode.');
    lines.push('');
  }

  lines.push('Tooling:');
  lines.push(`TypeScript version: ${comparison.tooling.typescriptVersion}`);
  lines.push(`TypeScript command: ${comparison.tooling.typescriptCommand}`);
  lines.push(`typescript-rust command: ${comparison.tooling.typescriptRustCommand}`);
  lines.push('');
  lines.push('Totals:');
  lines.push(`TypeScript diagnostics: ${comparison.typescript.total}`);
  lines.push(`typescript-rust diagnostics: ${comparison.typescriptRust.total}`);
  lines.push('');
  appendTriageSection(lines, comparison);
  lines.push('');
  lines.push('By code:');
  appendBucketSection(
    lines,
    comparison.matches.byCode,
    comparison.matches.onlyTypeScript,
    comparison.matches.onlyTypeScriptRust,
  );
  lines.push('');
  lines.push('By file/code:');
  appendBucketSection(
    lines,
    comparison.matches.byFileCode,
    comparison.matches.onlyTypeScriptFileCode,
    comparison.matches.onlyTypeScriptRustFileCode,
  );
  lines.push('');
  lines.push('By file/code/line:');
  if (comparison.summary.byFileCodeLineMatch === null) {
    lines.push('  (no line information on both sides)');
  } else {
    appendBucketSection(
      lines,
      comparison.matches.byFileCodeLine,
      comparison.matches.onlyTypeScriptFileCodeLine,
      comparison.matches.onlyTypeScriptRustFileCodeLine,
    );
  }
  lines.push('');
  lines.push('Summary:');
  lines.push(`Code-count match: ${comparison.summary.byCodeMatch ? 'yes' : 'no'}`);
  lines.push(`File/code match: ${comparison.summary.byFileCodeMatch ? 'yes' : 'no'}`);
  lines.push(
    `File/code/line match: ${
      comparison.summary.byFileCodeLineMatch === null
        ? 'n/a'
        : comparison.summary.byFileCodeLineMatch
          ? 'yes'
          : 'no'
    }`,
  );
  return `${lines.join('\n')}\n`;
}

function appendTriageSection(lines: string[], comparison: ComparisonResult): void {
  lines.push('Triage:');

  if (comparison.typescript.total > 0 && comparison.typescriptRust.total === 0) {
    lines.push(
      `  Project/file discovery problems: likely blocker (${comparison.typescript.total} TypeScript diagnostics, 0 rust diagnostics)`,
    );
    lines.push('  Parser unsupported syntax problems: deferred until source loading is fixed');
    lines.push('  Module resolution/package/import problems: deferred until source loading is fixed');
    lines.push('  Missing lib/@types/global problems: deferred until source loading is fixed');
    lines.push('  Semantic checker deltas: deferred until source loading is fixed');
    return;
  }

  const onlyTypeScriptBuckets = comparison.matches.onlyTypeScript;
  const parserCount = countBucketsByCode(onlyTypeScriptBuckets, PARSER_TRIAGE_CODES);
  const moduleCount = countBucketsByCode(onlyTypeScriptBuckets, MODULE_TRIAGE_CODES);
  const globalCount = countBucketsByCode(onlyTypeScriptBuckets, GLOBAL_TRIAGE_CODES);
  const semanticCount = Math.max(
    0,
    countBucketEntries(onlyTypeScriptBuckets) - parserCount - moduleCount - globalCount,
  );

  lines.push(`  Project/file discovery problems: ${comparison.typescriptRust.total === 0 ? comparison.typescript.total : 0}`);
  lines.push(`  Parser unsupported syntax problems: ${parserCount}`);
  lines.push(`  Module resolution/package/import problems: ${moduleCount}`);
  lines.push(`  Missing lib/@types/global problems: ${globalCount}`);
  lines.push(`  Semantic checker deltas: ${semanticCount}`);
}

function countBucketEntries(buckets: CountBucket[]): number {
  return buckets.reduce((total, bucket) => total + bucket.typescript, 0);
}

function countBucketsByCode(buckets: CountBucket[], codes: Set<string>): number {
  return buckets
    .filter((bucket) => codes.has(bucket.key))
    .reduce((total, bucket) => total + bucket.typescript, 0);
}

const PARSER_TRIAGE_CODES = new Set([
  'TS1005',
  'TS1109',
  'TS1128',
  'TS1134',
  'TS1160',
  'TS1161',
  'TS1206',
  'TS1308',
  'TS1434',
  'TS1435',
  'TS1443',
  'TS1450',
  'TS1451',
  'TS1472',
  'TS17008',
  'TS17009',
]);

const MODULE_TRIAGE_CODES = new Set([
  'TS2306',
  'TS2307',
  'TS2664',
  'TS2671',
  'TS2792',
  'TS2794',
  'TS5097',
]);

const GLOBAL_TRIAGE_CODES = new Set([
  'TS2304',
  'TS2552',
  'TS2580',
  'TS2584',
  'TS2686',
  'TS2688',
  'TS7016',
  'TS7017',
]);

export function appendBucketSection(
  lines: string[],
  matches: CountBucket[],
  onlyTypeScript: CountBucket[],
  onlyTypeScriptRust: CountBucket[],
): void {
  if (matches.length === 0 && onlyTypeScript.length === 0 && onlyTypeScriptRust.length === 0) {
    lines.push('  (none)');
    return;
  }

  for (const bucket of matches) {
    lines.push(`MATCH ${formatBucketKey(bucket.key)} ${bucket.typescript}`);
  }

  for (const bucket of onlyTypeScript) {
    if (bucket.typescriptRust === 0) {
      lines.push(`ONLY_TS ${formatBucketKey(bucket.key)} ${bucket.typescript}`);
    } else {
      lines.push(
        `DIFF ${formatBucketKey(bucket.key)} TypeScript=${bucket.typescript} typescript-rust=${bucket.typescriptRust}`,
      );
    }
  }

  for (const bucket of onlyTypeScriptRust) {
    if (bucket.typescript === 0) {
      lines.push(`ONLY_RUST ${formatBucketKey(bucket.key)} ${bucket.typescriptRust}`);
    } else {
      lines.push(
        `DIFF ${formatBucketKey(bucket.key)} TypeScript=${bucket.typescript} typescript-rust=${bucket.typescriptRust}`,
      );
    }
  }
}

export function formatBucketKey(key: string): string {
  const parts = key.split(' :: ');
  if (parts.length === 1) {
    return parts[0];
  }
  if (parts.length === 2) {
    return `${parts[0]} ${parts[1]}`;
  }
  return `${parts[0]} ${parts[1]} ${parts[2]}`;
}

export function displayProjectPath(tsconfigPath: string): string {
  const relative = path.relative(workspaceRoot, tsconfigPath);
  return relative.startsWith('..') ? normalizePathForDisplay(tsconfigPath) : normalizePathForDisplay(relative);
}

function printHelpAndExit(): never {
  process.stdout.write(
    [
      'Usage:',
      '  pnpm run oracle:compare -- --project <tsconfig.json|preset>',
      '  pnpm run oracle:compare -- --file <source.ts>',
      '',
      'Options:',
      '  --project <path|preset>   Compare a tsconfig file or known fixture preset.',
      '  --file <path>             Compare a single TypeScript source file.',
      '  --fixture <preset>        Alias for --project when passing a preset name.',
      '  --maxDiagnostics <n>      Limit diagnostics on both sides before comparing.',
      '  --json                    Emit machine-readable comparison output.',
      '  --failOnMismatch          Exit with code 1 when code/file mismatches exist.',
      '  --strictCodes             Alias for --failOnMismatch.',
      '',
      'Known presets:',
      `  ${Object.keys(fixturePresets).join(', ')}`,
      '',
      'Project mode examples:',
      '  pnpm run oracle:compare -- --project generics-basic',
      '  pnpm run oracle:compare -- --project tests/compat-projects/generics-basic/tsconfig.json',
      '',
      'File mode examples:',
      '  pnpm run oracle:compare -- --file examples/basic.ts',
      '  pnpm run oracle:compare -- --file examples/assignment.ts',
      '',
    ].join('\n'),
  );
  process.exit(0);
}

function formatParseFailure(output: string, error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  return [`Parse error: ${message}`, 'Output:', output.trim() || '(empty)'].join('\n');
}

function readPinnedTypeScriptVersion(): string {
  const packageJsonPath = path.join(workspaceRoot, 'package.json');
  const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf8')) as {
    devDependencies?: Record<string, string>;
  };

  return packageJson.devDependencies?.typescript ?? 'unknown';
}

function looksLikePath(value: string): boolean {
  return value.includes('/') || value.includes('\\') || value.endsWith('.json') || value.startsWith('.');
}

export function isSourceFilePath(value: string): boolean {
  return ['.ts', '.tsx', '.js', '.mts', '.cts'].includes(path.extname(value).toLowerCase());
}

export function isTsConfigPath(value: string): boolean {
  return path.basename(normalizePathForDisplay(value)).toLowerCase() === 'tsconfig.json';
}

export function resolveWorkspacePath(value: string): string {
  return path.isAbsolute(value) ? value : path.resolve(workspaceRoot, value);
}

function isAbsolutePathLike(value: string): boolean {
  const normalized = normalizePathForDisplay(value);
  return (
    path.isAbsolute(value) ||
    path.win32.isAbsolute(value) ||
    normalized.startsWith('/') ||
    /^[A-Za-z]:\//.test(normalized) ||
    normalized.startsWith('//')
  );
}

function executeComparison(
  mode: OracleMode,
  maxDiagnostics?: number,
): ComparisonResult {
  const comparisonPath = mode.kind === 'project' ? mode.resolvedTsconfig : mode.resolvedFile;
  const comparisonDisplay = displayComparisonTargetPath(comparisonPath);
  const projectDir = path.dirname(comparisonPath);
  const tsc = runTsc(mode);
  const rust = runTypeScriptRust(mode, maxDiagnostics);
  const rustOutput = rust.stdout.trim() ? rust.stdout : rust.stderr;

  const tscDiagnostics = limitDiagnostics(
    parseTypeScriptDiagnostics(`${tsc.stdout}${tsc.stderr}`, projectDir),
    maxDiagnostics,
  );
  const rustDiagnostics = limitDiagnostics(parseTypeScriptRustDiagnostics(rustOutput, projectDir), maxDiagnostics);

  return compareDiagnostics(mode.kind, comparisonDisplay, tscDiagnostics, rustDiagnostics, mode.ignoreConfig, mode.stubExternalModules);
}

export function displayComparisonTargetPath(targetPath: string): string {
  return displayPath(targetPath);
}

function displayPath(targetPath: string): string {
  const relative = path.relative(workspaceRoot, targetPath);
  return relative.startsWith('..') ? normalizePathForDisplay(targetPath) : normalizePathForDisplay(relative);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  try {
    main();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`${message}\n`);
    process.exit(1);
  }
}
