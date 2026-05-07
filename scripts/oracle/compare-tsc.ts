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

export type DiagnosticFingerprint = {
  fileName: string;
  code: string;
  line: number | null;
  column: number | null;
  message: string | null;
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

export type CategorizedCountEntry = {
  key: string;
  category: string;
  count: number;
};

export type ModuleExportCountEntry = {
  moduleSpecifier: string;
  exportName: string;
  category: string;
  count: number;
};

export type TsconfigPathMapping = {
  pattern: string;
  substitutions: string[];
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
    rustJobs?: number;
  };
  warnings?: string[];
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
  details?: {
    onlyTypeScriptRust?: {
      ts2305ByModuleAndExport?: ModuleExportCountEntry[];
      ts2307ByModuleSpecifier?: CategorizedCountEntry[];
      ts2304ByIdentifier?: CategorizedCountEntry[];
      nodeModulesSourceDiagnosticsByPrefix?: CountEntry[];
      nodeModulesJavaScriptSourceDiagnosticsByPrefix?: CountEntry[];
    };
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
  rustJobs?: number;
};

export type OracleMode =
  | {
      kind: 'project';
      project: string;
      resolvedTsconfig: string;
      ignoreConfig?: boolean;
      stubExternalModules?: boolean;
      rustJobs?: number;
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
  'module-export-visibility-hardening': path.join(workspaceRoot, 'tests/compat-projects/module-export-visibility-hardening/tsconfig.json'),
  'declaration-reexports-hardening': path.join(workspaceRoot, 'tests/compat-projects/declaration-reexports-hardening/tsconfig.json'),
  'package-exports-types-hardening': path.join(workspaceRoot, 'tests/compat-projects/package-exports-types-hardening/tsconfig.json'),
  'diagnostics-pack': path.join(workspaceRoot, 'tests/compat-projects/diagnostics-pack/tsconfig.json'),
  'generics-basic': path.join(workspaceRoot, 'tests/compat-projects/generics-basic/tsconfig.json'),
  'relative-js-extension-substitution-basic': path.join(workspaceRoot, 'tests/compat-projects/relative-js-extension-substitution-basic/tsconfig.json'),
  'relative-directory-index-basic': path.join(workspaceRoot, 'tests/compat-projects/relative-directory-index-basic/tsconfig.json'),
  'import-graph-generated-relative-basic': path.join(workspaceRoot, 'tests/compat-projects/import-graph-generated-relative-basic/tsconfig.json'),
  'paths-wildcard-import-graph-basic': path.join(workspaceRoot, 'tests/compat-projects/paths-wildcard-import-graph-basic/tsconfig.json'),
  'dependency-incomplete-declaration-export-fallback': path.join(workspaceRoot, 'tests/compat-projects/dependency-incomplete-declaration-export-fallback/tsconfig.json'),
  'skip-lib-check-dependency-dts': path.join(workspaceRoot, 'tests/compat-projects/skip-lib-check-dependency-dts/tsconfig.json'),
  'skip-lib-check-local-dts': path.join(workspaceRoot, 'tests/compat-projects/skip-lib-check-local-dts/tsconfig.json'),
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
      ? compareProject(
          mode.resolvedTsconfig,
          displayComparisonTargetPath(mode.resolvedTsconfig),
          args.maxDiagnostics,
          mode.stubExternalModules,
          mode.rustJobs,
        )
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
    } else if (arg === '--rustJobs') {
      const value = argv[++index];
      if (!value) {
        throw new Error('--rustJobs requires a value');
      }
      const parsedValue = Number(value);
      if (!Number.isInteger(parsedValue) || parsedValue <= 0) {
        throw new Error('--rustJobs must be greater than 0');
      }
      parsed.rustJobs = parsedValue;
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
      rustJobs: args.rustJobs,
    };
  }

  if (args.rustJobs !== undefined) {
    throw new Error('--rustJobs is only supported with --project.');
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
  rustJobs?: number,
): ComparisonResult {
  return executeComparison(
    {
      kind: 'project',
      project: projectDisplay,
      resolvedTsconfig: tsconfigPath,
      stubExternalModules,
      rustJobs,
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

export function runTypeScriptRust(
  mode: OracleMode,
  maxDiagnostics?: number,
  rustJobs?: number,
): RunResult {
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
    if (rustJobs !== undefined) {
      args.push('--jobs', String(rustJobs));
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

export function normalizeDiagnostic(diagnostic: NormalizedDiagnostic): DiagnosticFingerprint {
  return {
    fileName: diagnostic.fileName,
    code: diagnostic.code,
    line: diagnostic.line ?? null,
    column: diagnostic.column ?? null,
    message: diagnostic.message ?? null,
  };
}

export function normalizeDiagnostics(diagnostics: NormalizedDiagnostic[]): DiagnosticFingerprint[] {
  return diagnostics.map(normalizeDiagnostic);
}

export function compareDiagnostics(
  mode: 'project' | 'file',
  targetDisplay: string,
  typescript: NormalizedDiagnostic[],
  typescriptRust: NormalizedDiagnostic[],
  ignoreConfig?: boolean,
  stubExternalModules?: boolean,
  projectRoot?: string,
  pathsMappings: TsconfigPathMapping[] = [],
  rustJobs?: number,
): ComparisonResult {
  const byCode = compareBuckets(typescript, typescriptRust, keyByCode);
  const byFileCode = compareBuckets(typescript, typescriptRust, keyByFileCode);
  const byFileCodeLine = compareBuckets(
    typescript.filter(hasLineInfo),
    typescriptRust.filter(hasLineInfo),
    keyByFileCodeLine,
  );
  const onlyLineDiagnostics = subtractDiagnosticsByKey(
    typescript.filter(hasLineInfo),
    typescriptRust.filter(hasLineInfo),
    keyByFileCodeLine,
  );
  const onlyTypeScriptRustDiagnostics = onlyLineDiagnostics.onlyRight;

  return {
    mode,
    project: mode === 'project' ? targetDisplay : null,
    file: mode === 'file' ? targetDisplay : null,
    ignoreConfig: ignoreConfig ?? false,
    typescriptRustOptions: {
      stubExternalModules: stubExternalModules ?? false,
      rustJobs,
    },
    warnings: buildComparisonWarnings(typescript, typescriptRust),
    tooling: {
      typescriptVersion: pinnedTypeScriptVersion,
      typescriptCommand: buildTypeScriptCommand(mode, targetDisplay, ignoreConfig),
      typescriptRustCommand: buildTypeScriptRustCommand(
        mode,
        targetDisplay,
        ignoreConfig,
        stubExternalModules,
        rustJobs,
      ),
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
    details: {
      onlyTypeScriptRust: {
        ts2305ByModuleAndExport: groupDiagnosticsByModuleExportExtractor(
          onlyTypeScriptRustDiagnostics.filter((diagnostic) => diagnostic.code === 'TS2305'),
          (diagnostic) => {
            const exportInfo = extractTs2305ModuleExport(diagnostic.message);
            if (!exportInfo) {
              return null;
            }

            return {
              moduleSpecifier: exportInfo.moduleSpecifier,
              exportName: exportInfo.exportName,
              category: classifyTs2305ModuleExport(
                exportInfo.moduleSpecifier,
                diagnostic.fileName,
                projectRoot,
                pathsMappings,
              ),
            };
          },
        ),
        ts2307ByModuleSpecifier: groupDiagnosticsByCategorizedExtractor(
          onlyTypeScriptRustDiagnostics.filter((diagnostic) => diagnostic.code === 'TS2307'),
          (diagnostic) => {
            const specifier = extractTs2307ModuleSpecifier(diagnostic.message);
            return specifier
              ? {
                  key: specifier,
                  category: classifyTs2307ModuleSpecifier(specifier, diagnostic.fileName, projectRoot, pathsMappings),
                }
              : null;
          },
        ),
        ts2304ByIdentifier: groupDiagnosticsByCategorizedExtractor(
          onlyTypeScriptRustDiagnostics.filter((diagnostic) => diagnostic.code === 'TS2304'),
          (diagnostic) => {
            const identifier = extractTs2304Identifier(diagnostic.message);
            return identifier
              ? {
                  key: identifier,
                  category: classifyTs2304Identifier(identifier),
                }
              : null;
          },
        ),
        nodeModulesSourceDiagnosticsByPrefix: groupDiagnosticsByKey(
          onlyTypeScriptRustDiagnostics.filter((diagnostic) => isNodeModulesSourceDiagnostic(diagnostic)),
          (diagnostic) => nodeModulesSourcePrefix(diagnostic.fileName) ?? diagnostic.fileName,
        ),
        nodeModulesJavaScriptSourceDiagnosticsByPrefix: groupDiagnosticsByKey(
          onlyTypeScriptRustDiagnostics.filter((diagnostic) =>
            isNodeModulesJavaScriptSourceDiagnostic(diagnostic),
          ),
          (diagnostic) => nodeModulesSourcePrefix(diagnostic.fileName) ?? diagnostic.fileName,
        ),
      },
    },
  };
}

export function buildTypeScriptCommand(mode: 'project' | 'file', targetDisplay: string, ignoreConfig?: boolean): string {
  if (mode === 'project') {
    return `pnpm exec tsc --noEmit --pretty false --project ${targetDisplay}`;
  }

  return ignoreConfig ? `pnpm exec tsc --noEmit --pretty false --ignoreConfig ${targetDisplay}` : `pnpm exec tsc --noEmit --pretty false ${targetDisplay}`;
}

export function buildTypeScriptRustCommand(
  mode: 'project' | 'file',
  targetDisplay: string,
  ignoreConfig?: boolean,
  stubExternalModules?: boolean,
  rustJobs?: number,
): string {
  const cargoToml = normalizePathForDisplay(path.join(workspaceRoot, 'Cargo.toml'));
  let args = `cargo run -q --manifest-path ${cargoToml} -p typescript-rust-cli --`;

  if (mode === 'project') {
    args += ` --project ${targetDisplay} --format json`;
    if (stubExternalModules) {
      args += ` --stubExternalModules`;
    }
    if (rustJobs !== undefined) {
      args += ` --jobs ${rustJobs}`;
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

export function subtractDiagnosticsByKey(
  left: NormalizedDiagnostic[],
  right: NormalizedDiagnostic[],
  keyFn: (diagnostic: NormalizedDiagnostic) => string,
): {
  onlyLeft: NormalizedDiagnostic[];
  onlyRight: NormalizedDiagnostic[];
} {
  const leftRemaining = countDiagnostics(left, keyFn);
  const rightRemaining = countDiagnostics(right, keyFn);
  const onlyLeft: NormalizedDiagnostic[] = [];
  const onlyRight: NormalizedDiagnostic[] = [];

  for (const diagnostic of left) {
    const key = keyFn(diagnostic);
    const remaining = rightRemaining.get(key) ?? 0;
    if (remaining > 0) {
      rightRemaining.set(key, remaining - 1);
    } else {
      onlyLeft.push(diagnostic);
    }
  }

  for (const diagnostic of right) {
    const key = keyFn(diagnostic);
    const remaining = leftRemaining.get(key) ?? 0;
    if (remaining > 0) {
      leftRemaining.set(key, remaining - 1);
    } else {
      onlyRight.push(diagnostic);
    }
  }

  return { onlyLeft, onlyRight };
}

export function groupDiagnosticsByKey(
  diagnostics: NormalizedDiagnostic[],
  keyFn: (diagnostic: NormalizedDiagnostic) => string,
): CountEntry[] {
  return countEntriesFromCounts(countDiagnostics(diagnostics, keyFn));
}

export function groupDiagnosticsByExtractor(
  diagnostics: NormalizedDiagnostic[],
  extractor: (diagnostic: NormalizedDiagnostic) => string | null,
): CountEntry[] {
  const counts = new Map<string, number>();

  for (const diagnostic of diagnostics) {
    const key = extractor(diagnostic);
    if (!key) {
      continue;
    }

    counts.set(key, (counts.get(key) ?? 0) + 1);
  }

  return countEntriesFromCounts(counts);
}

export function groupDiagnosticsByModuleExportExtractor(
  diagnostics: NormalizedDiagnostic[],
  extractor: (
    diagnostic: NormalizedDiagnostic,
  ) => { moduleSpecifier: string; exportName: string; category: string } | null,
): ModuleExportCountEntry[] {
  const counts = new Map<string, ModuleExportCountEntry>();

  for (const diagnostic of diagnostics) {
    const bucket = extractor(diagnostic);
    if (!bucket) {
      continue;
    }

    const dedupeKey = `${bucket.moduleSpecifier} :: ${bucket.exportName}`;
    const existing = counts.get(dedupeKey);
    if (existing) {
      existing.count += 1;
      continue;
    }

    counts.set(dedupeKey, { ...bucket, count: 1 });
  }

  return [...counts.values()].sort(
    (left, right) =>
      right.count - left.count ||
      left.moduleSpecifier.localeCompare(right.moduleSpecifier) ||
      left.exportName.localeCompare(right.exportName),
  );
}

export function groupDiagnosticsByCategorizedExtractor(
  diagnostics: NormalizedDiagnostic[],
  extractor: (diagnostic: NormalizedDiagnostic) => { key: string; category: string } | null,
): CategorizedCountEntry[] {
  const counts = new Map<string, CategorizedCountEntry>();

  for (const diagnostic of diagnostics) {
    const bucket = extractor(diagnostic);
    if (!bucket) {
      continue;
    }

    const dedupeKey = `${bucket.key} :: ${bucket.category}`;
    const existing = counts.get(dedupeKey);
    if (existing) {
      existing.count += 1;
      continue;
    }

    counts.set(dedupeKey, { ...bucket, count: 1 });
  }

  return [...counts.values()].sort(
    (left, right) =>
      right.count - left.count || left.key.localeCompare(right.key) || left.category.localeCompare(right.category),
  );
}

export function extractTs2307ModuleSpecifier(message?: string): string | null {
  if (!message) {
    return null;
  }

  const match = message.match(/module ['"]([^'"]+)['"]/i);
  return match ? match[1] : null;
}

export function extractTs2305ModuleExport(
  message?: string,
): { moduleSpecifier: string; exportName: string } | null {
  if (!message) {
    return null;
  }

  const match = message.match(
    /Module ['"]([^'"]+)['"] has no exported member ['"]([^'"]+)['"]/i,
  );
  return match ? { moduleSpecifier: match[1], exportName: match[2] } : null;
}

// These are triage categories for reporting, not semantic claims about the
// underlying compiler behavior.
export function classifyTs2305ModuleExport(
  moduleSpecifier: string,
  diagnosticFileName?: string,
  projectRoot?: string,
  pathsMappings: TsconfigPathMapping[] = [],
): string {
  if (isRelativeSpecifier(moduleSpecifier)) {
    const resolvedPath = resolveRelativeCandidateForReporting(
      diagnosticFileName ?? '',
      moduleSpecifier,
      projectRoot,
    );
    return resolvedPath ? classifyLoadedModulePath(resolvedPath) : 'unknown';
  }

  const pathAliasCategory = classifyPathsAliasModuleSpecifier(moduleSpecifier, projectRoot, pathsMappings);
  if (pathAliasCategory === 'paths-alias-explicit-relative-target') {
    const resolvedPath = resolvePathsAliasCandidate(moduleSpecifier, projectRoot, pathsMappings);
    return resolvedPath ? classifyLoadedModulePath(resolvedPath) : 'unknown';
  }

  const packageName = packageNameFromSpecifier(moduleSpecifier);
  if (packageName && projectRoot) {
    const resolvedPath = resolvePackageDeclarationCandidate(packageName, projectRoot);
    if (resolvedPath) {
      return hasIncompleteDeclarationSurface(resolvedPath)
        ? 'package-derived-incomplete-declaration'
        : 'dependency-declaration-module';
    }
  }

  return 'unknown';
}

function isDeclarationFileName(fileName: string): boolean {
  const lower = fileName.toLowerCase();
  return lower.endsWith('.d.ts') || lower.endsWith('.d.mts') || lower.endsWith('.d.cts');
}

function isDependencyDeclarationPath(fileName: string): boolean {
  const lower = fileName.toLowerCase();
  return isDeclarationFileName(fileName) && (lower.includes('/node_modules/') || lower.includes('\\node_modules\\'));
}

function isJsonModuleSpecifier(specifier: string): boolean {
  return specifier.toLowerCase().endsWith('.json');
}

function isGeneratedModuleSpecifier(specifier: string): boolean {
  const lower = specifier.toLowerCase();
  return lower.includes('.gen') || lower.includes('/generated/');
}

function isConfigToolingModuleSpecifier(specifier: string): boolean {
  const lower = specifier.toLowerCase();
  return (
    lower.endsWith('.config') ||
    lower.endsWith('.config.ts') ||
    lower.endsWith('.config.tsx') ||
    lower.endsWith('.config.mts') ||
    lower.endsWith('.config.cts') ||
    lower.endsWith('.config.js') ||
    lower.endsWith('.config.mjs') ||
    lower.endsWith('.config.cjs') ||
    lower.includes('.config/') ||
    lower.includes('/config.') ||
    lower.includes('/config/') ||
    lower.includes('vitest.config') ||
    lower.includes('eslint.config') ||
    lower.includes('tailwind.config') ||
    lower.includes('next.config') ||
    lower.includes('postcss.config') ||
    lower.includes('drizzle.config') ||
    lower.includes('sandbox.config') ||
    lower.includes('playwright.config') ||
    lower.includes('turbo.json') ||
    lower.endsWith('package.json') ||
    lower.endsWith('tsconfig.json') ||
    lower.endsWith('deno.json') ||
    lower.endsWith('vercel.json') ||
    lower.endsWith('package-lock.json')
  );
}

function isRelativeSpecifier(specifier: string): boolean {
  return (
    specifier === '.' ||
    specifier === '..' ||
    specifier.startsWith('./') ||
    specifier.startsWith('../') ||
    specifier.startsWith('.\\') ||
    specifier.startsWith('..\\')
  );
}

function isPackageSubpathSpecifier(specifier: string): boolean {
  if (!specifier.includes('/')) {
    return false;
  }

  if (!specifier.startsWith('@')) {
    return true;
  }

  return specifier.split('/').length > 2;
}

function isJsxLikeIdentifier(identifier: string): boolean {
  return ['JSX', 'IntrinsicElements', 'Fragment', 'React'].includes(identifier);
}

function isDomLikeIdentifier(identifier: string): boolean {
  return new Set([
    'document',
    'window',
    'navigator',
    'Headers',
    'FormData',
    'URLSearchParams',
    'Blob',
    'File',
    'Response',
    'Request',
    'ReadableStream',
    'WritableStream',
    'TransformStream',
    'Event',
    'MessageEvent',
    'HTMLElement',
    'Element',
    'Node',
    'Text',
    'Document',
  ]).has(identifier);
}

function isNodeLikeIdentifier(identifier: string): boolean {
  return ['process', 'Buffer', 'require', 'module', 'exports', '__dirname', '__filename'].includes(identifier);
}

function isGenericOrTypeParameterScopeIdentifier(identifier: string): boolean {
  return ['T', 'K', 'V', 'U', 'P', 'R', 'S', 'E', 'A', 'B', 'C', 'D', 'M', 'N', 'O'].includes(identifier);
}

function isMissingSyntheticLibGlobalIdentifier(identifier: string): boolean {
  return [
    'Array',
    'String',
    'Number',
    'Boolean',
    'Symbol',
    'Promise',
    'Date',
    'RegExp',
    'Error',
    'Math',
    'JSON',
    'Intl',
    'Console',
    'console',
    'Record',
    'ReadonlyArray',
  ].includes(identifier);
}

function isMissingEsLibLiteGlobalIdentifier(identifier: string): boolean {
  return [
    'Object',
    'Map',
    'Set',
    'WeakMap',
    'WeakSet',
    'Uint8Array',
    'Uint16Array',
    'Uint32Array',
    'Int8Array',
    'Int16Array',
    'Int32Array',
    'Float32Array',
    'Float64Array',
    'BigInt64Array',
    'BigUint64Array',
    'globalThis',
    'isNaN',
  ].includes(identifier);
}

function isLocalUnresolvedIdentifier(identifier: string): boolean {
  const first = identifier[0];
  return Boolean(first && (/[a-z_]/.test(first) || /\d/.test(first)));
}

function isPackageDerivedIdentifier(identifier: string): boolean {
  const first = identifier[0];
  return Boolean(first && identifier.length > 1 && /[A-Z]/.test(first));
}

export function extractTs2304Identifier(message?: string): string | null {
  if (!message) {
    return null;
  }

  const match = message.match(/Cannot find (?:name|namespace) ['"]([^'"]+)['"]/i);
  return match ? match[1] : null;
}

export function classifyTs2307ModuleSpecifier(
  specifier: string,
  diagnosticFileName?: string,
  projectRoot?: string,
  pathsMappings: TsconfigPathMapping[] = [],
): string {
  if (isJsonModuleSpecifier(specifier)) {
    return 'package-json';
  }
  if (isGeneratedModuleSpecifier(specifier)) {
    if (diagnosticFileName && projectRoot) {
      const resolvedPath = resolveRelativeCandidateForReporting(diagnosticFileName, specifier, projectRoot);
      if (resolvedPath) {
        return resolvedPath ? 'relative-generated-existing-not-loaded' : 'relative-generated-missing';
      }
    }
    return 'relative-generated-missing';
  }
  if (isConfigToolingModuleSpecifier(specifier)) {
    return 'package-json';
  }
  if (isRelativeSpecifier(specifier)) {
    if (diagnosticFileName && projectRoot) {
      const resolvedPath = resolveRelativeCandidateForReporting(diagnosticFileName, specifier, projectRoot);
      if (resolvedPath) {
        return 'relative-existing-not-loaded';
      }
    }
    return 'relative-missing';
  }
  const pathAliasCategory = classifyPathsAliasModuleSpecifier(specifier, projectRoot, pathsMappings);
  if (pathAliasCategory) {
    return pathAliasCategory;
  }
  if (isNodeBuiltinModuleSpecifier(specifier)) {
    return 'node-builtin';
  }
  if (isPackageJsonModuleSpecifier(specifier)) {
    return 'package-json';
  }
  if (isPackageSubpathSpecifier(specifier)) {
    return 'package-subpath';
  }
  return 'package';
}

export function classifyTs2304Identifier(identifier: string): string {
  if (isJsxLikeIdentifier(identifier)) {
    return 'jsx-like';
  }
  if (isDomLikeIdentifier(identifier)) {
    return 'dom-like';
  }
  if (isNodeLikeIdentifier(identifier)) {
    return 'missing-node-like-global';
  }
  if (isGenericOrTypeParameterScopeIdentifier(identifier)) {
    return 'generic-or-type-parameter-scope';
  }
  if (isMissingSyntheticLibGlobalIdentifier(identifier)) {
    return 'missing-synthetic-built-in';
  }
  if (isMissingEsLibLiteGlobalIdentifier(identifier)) {
    return 'missing-es-lib-lite-global';
  }
  if (isPackageDerivedIdentifier(identifier)) {
    return 'package-derived-incomplete-declaration';
  }
  if (isLocalUnresolvedIdentifier(identifier)) {
    return 'local-unresolved';
  }
  return 'unknown';
}

function classifyPathsAliasModuleSpecifier(
  specifier: string,
  projectRoot: string | undefined,
  pathsMappings: TsconfigPathMapping[],
): string | null {
  for (const mapping of pathsMappings) {
    const wildcardIndex = mapping.pattern.indexOf('*');
    const isWildcard = wildcardIndex !== -1;

    let matched = false;
    if (isWildcard) {
      const parts = mapping.pattern.split('*');
      if (parts.length !== 2) {
        continue;
      }

      const prefix = parts[0];
      const suffix = parts[1];
      matched =
        specifier.startsWith(prefix) &&
        specifier.endsWith(suffix) &&
        specifier.length >= prefix.length + suffix.length;
    } else {
      matched = specifier === mapping.pattern;
    }

    if (!matched) {
      continue;
    }

    if (mapping.substitutions.some((substitution) => isExplicitRelativeTarget(substitution))) {
      return 'paths-alias-explicit-relative-target';
    }

    return 'paths-alias-unsupported-baseUrl-dependent-target';
  }

  return null;
}

function classifyLoadedModulePath(resolvedPath: string): string {
  if (isDependencyDeclarationPath(resolvedPath)) {
    return hasIncompleteDeclarationSurface(resolvedPath)
      ? 'package-derived-incomplete-declaration'
      : 'dependency-declaration-module';
  }

  if (isDeclarationFileName(resolvedPath)) {
    return 'local-declaration-module';
  }

  return 'source-module';
}

function resolveRelativeCandidateForReporting(
  importerFileName: string,
  specifier: string,
  projectRoot?: string,
): string | null {
  if (!projectRoot) {
    return null;
  }

  const importerPath = path.isAbsolute(importerFileName)
    ? importerFileName
    : path.resolve(projectRoot, importerFileName);
  const importerDir = path.dirname(importerPath);
  const normalizedSpecifier = normalizePathForDisplay(specifier);
  const joined = normalizePathForDisplay(path.join(importerDir, normalizedSpecifier));
  const candidatePaths = relativeResolutionCandidatesForSpecifier(joined, normalizedSpecifier);

  for (const candidate of candidatePaths) {
    if (existsSync(candidate) && statSync(candidate).isFile()) {
      return candidate;
    }
  }

  return null;
}

function resolvePathsAliasCandidate(
  specifier: string,
  projectRoot: string | undefined,
  pathsMappings: TsconfigPathMapping[],
): string | null {
  if (!projectRoot) {
    return null;
  }

  for (const mapping of pathsMappings) {
    const matchedText = matchPathMappingSpecifier(mapping.pattern, specifier);
    if (matchedText === null) {
      continue;
    }

    for (const substitution of mapping.substitutions) {
      if (!isExplicitRelativeTarget(substitution)) {
        continue;
      }

      const targetPath = mapping.pattern.includes('*')
        ? substitution.replace('*', matchedText)
        : substitution;
      if (relativeSpecifierKind(targetPath) === 'unsupported') {
        continue;
      }
      const absoluteTarget = path.resolve(projectRoot, targetPath);
      const candidatePaths = relativeResolutionCandidatesForSpecifier(absoluteTarget, targetPath);

      for (const candidate of candidatePaths) {
        if (existsSync(candidate) && statSync(candidate).isFile()) {
          return candidate;
        }
      }
    }
  }

  return null;
}

function matchPathMappingSpecifier(pattern: string, specifier: string): string | null {
  if (pattern.indexOf('*') === -1) {
    return specifier === pattern ? '' : null;
  }

  const parts = pattern.split('*');
  if (parts.length !== 2) {
    return null;
  }

  const prefix = parts[0];
  const suffix = parts[1];
  if (
    !specifier.startsWith(prefix) ||
    !specifier.endsWith(suffix) ||
    specifier.length < prefix.length + suffix.length
  ) {
    return null;
  }

  return specifier.slice(prefix.length, specifier.length - suffix.length);
}

function packageNameFromSpecifier(specifier: string): string | null {
  if (specifier.startsWith('@')) {
    const parts = specifier.split('/');
    return parts.length >= 2 ? `${parts[0]}/${parts[1]}` : null;
  }

  const slashIndex = specifier.indexOf('/');
  return slashIndex === -1 ? specifier : specifier.slice(0, slashIndex);
}

function resolvePackageDeclarationCandidate(packageName: string, projectRoot: string): string | null {
  const packageRoot = path.join(projectRoot, 'node_modules', packageName);
  const packageJson = readJsonIfExists(path.join(packageRoot, 'package.json'));
  const typesField =
    packageJson && typeof packageJson.types === 'string'
      ? packageJson.types
      : packageJson && typeof packageJson.typings === 'string'
        ? packageJson.typings
        : null;

  if (typesField) {
    const candidate = path.resolve(packageRoot, typesField);
    if (existsSync(candidate) && statSync(candidate).isFile()) {
      return candidate;
    }
  }

  for (const candidate of [
    path.join(packageRoot, 'index.d.ts'),
    path.join(packageRoot, 'types.d.ts'),
    path.join(packageRoot, 'typings.d.ts'),
  ]) {
    if (existsSync(candidate) && statSync(candidate).isFile()) {
      return candidate;
    }
  }

  return null;
}

function readJsonIfExists(pathname: string): Record<string, unknown> | null {
  if (!existsSync(pathname)) {
    return null;
  }

  try {
    return JSON.parse(readFileSync(pathname, 'utf8')) as Record<string, unknown>;
  } catch {
    return null;
  }
}

function hasIncompleteDeclarationSurface(resolvedPath: string): boolean {
  try {
    return hasIncompleteDeclarationSurfaceText(readFileSync(resolvedPath, 'utf8'));
  } catch {
    return false;
  }
}

function hasIncompleteDeclarationSurfaceText(sourceText: string): boolean {
  return /export\s*=\s*/.test(sourceText) || /declare\s+namespace\b/.test(sourceText) || /export\s+as\s+namespace\b/.test(sourceText);
}

function isExplicitRelativeTarget(target: string): boolean {
  return target.startsWith('./') || target.startsWith('../') || target.startsWith('.\\') || target.startsWith('..\\');
}

function relativeSpecifierKind(specifier: string): 'explicit-ts' | 'explicit-js' | 'explicit-mjs' | 'explicit-cjs' | 'extensionless' | 'unsupported' {
  const lastSegment = specifier.replace(/\\/g, '/').split('/').pop() ?? specifier;

  if (lastSegment === '.' || lastSegment === '..') {
    return 'extensionless';
  }

  if (
    lastSegment.endsWith('.tsx') ||
    lastSegment.endsWith('.jsx') ||
    lastSegment.endsWith('.mts') ||
    lastSegment.endsWith('.cts') ||
    lastSegment.endsWith('.d.ts') ||
    lastSegment.endsWith('.d.mts') ||
    lastSegment.endsWith('.d.cts') ||
    lastSegment.endsWith('.json')
  ) {
    return 'unsupported';
  }

  if (lastSegment.endsWith('.ts')) {
    return 'explicit-ts';
  }

  if (lastSegment.endsWith('.js')) {
    return 'explicit-js';
  }

  if (lastSegment.endsWith('.mjs')) {
    return 'explicit-mjs';
  }

  if (lastSegment.endsWith('.cjs')) {
    return 'explicit-cjs';
  }

  return 'extensionless';
}

function isPackageJsonModuleSpecifier(specifier: string): boolean {
  return specifier.toLowerCase().endsWith('.json');
}

function isNodeBuiltinModuleSpecifier(specifier: string): boolean {
  return new Set([
    'assert',
    'buffer',
    'child_process',
    'cluster',
    'console',
    'constants',
    'crypto',
    'dgram',
    'diagnostics_channel',
    'dns',
    'domain',
    'events',
    'fs',
    'http',
    'http2',
    'https',
    'inspector',
    'module',
    'net',
    'os',
    'path',
    'perf_hooks',
    'process',
    'punycode',
    'querystring',
    'readline',
    'repl',
    'stream',
    'string_decoder',
    'sys',
    'timers',
    'tls',
    'trace_events',
    'tty',
    'url',
    'util',
    'v8',
    'vm',
    'worker_threads',
    'zlib',
    'node:assert',
    'node:buffer',
    'node:child_process',
    'node:cluster',
    'node:console',
    'node:constants',
    'node:crypto',
    'node:dgram',
    'node:diagnostics_channel',
    'node:dns',
    'node:domain',
    'node:events',
    'node:fs',
    'node:http',
    'node:http2',
    'node:https',
    'node:inspector',
    'node:module',
    'node:net',
    'node:os',
    'node:path',
    'node:perf_hooks',
    'node:process',
    'node:punycode',
    'node:querystring',
    'node:readline',
    'node:repl',
    'node:stream',
    'node:string_decoder',
    'node:sys',
    'node:timers',
    'node:tls',
    'node:trace_events',
    'node:tty',
    'node:url',
    'node:util',
    'node:v8',
    'node:vm',
    'node:worker_threads',
    'node:zlib',
  ]).has(specifier);
}

function relativeResolutionCandidatesForSpecifier(base: string, specifier: string): string[] {
  const kind = relativeSpecifierKind(specifier);
  if (kind === 'unsupported') {
    return [];
  }

  const candidates = [base];

  if (kind === 'explicit-ts') {
    return candidates.map(normalizePathForDisplay);
  }

  if (kind === 'explicit-js') {
    const stripped = stripExtension(base);
    candidates.push(
      `${stripped}.ts`,
      `${stripped}.tsx`,
      `${stripped}.d.ts`,
      `${stripped}/index.ts`,
      `${stripped}/index.tsx`,
      `${stripped}/index.d.ts`,
    );
    return candidates.map(normalizePathForDisplay);
  }

  if (kind === 'explicit-mjs') {
    const stripped = stripExtension(base);
    candidates.push(`${stripped}.mts`, `${stripped}.d.mts`, `${stripped}/index.mts`, `${stripped}/index.d.mts`);
    return candidates.map(normalizePathForDisplay);
  }

  if (kind === 'explicit-cjs') {
    const stripped = stripExtension(base);
    candidates.push(`${stripped}.cts`, `${stripped}.d.cts`, `${stripped}/index.cts`, `${stripped}/index.d.cts`);
    return candidates.map(normalizePathForDisplay);
  }

  candidates.push(
    `${base}.ts`,
    `${base}.tsx`,
    `${base}.d.ts`,
    `${base}.mts`,
    `${base}.cts`,
    `${base}.d.mts`,
    `${base}.d.cts`,
    `${base}/index.ts`,
    `${base}/index.tsx`,
    `${base}/index.d.ts`,
    `${base}/index.mts`,
    `${base}/index.cts`,
    `${base}/index.d.mts`,
    `${base}/index.d.cts`,
  );

  return candidates.map(normalizePathForDisplay);
}

function stripExtension(value: string): string {
  const lastSlash = value.lastIndexOf('/');
  const lastDot = value.lastIndexOf('.');
  if (lastDot <= lastSlash) {
    return value;
  }

  return value.slice(0, lastDot);
}

export function isNodeModulesSourceDiagnostic(diagnostic: NormalizedDiagnostic): boolean {
  return diagnostic.fileName.includes('/node_modules/') && !isDeclarationFileName(diagnostic.fileName);
}

export function isNodeModulesJavaScriptSourceDiagnostic(diagnostic: NormalizedDiagnostic): boolean {
  if (!isNodeModulesSourceDiagnostic(diagnostic)) {
    return false;
  }

  return (
    diagnostic.fileName.endsWith('.js') ||
    diagnostic.fileName.endsWith('.jsx') ||
    diagnostic.fileName.endsWith('.mjs') ||
    diagnostic.fileName.endsWith('.cjs')
  );
}

export function nodeModulesSourcePrefix(fileName: string): string | null {
  const normalized = fileName.replace(/\\/g, '/');
  const needle = '/node_modules/';
  const index = normalized.indexOf(needle);
  if (index === -1) {
    return null;
  }

  const remainder = normalized.slice(index + needle.length);
  const segments = remainder.split('/');
  const first = segments[0];
  if (!first) {
    return null;
  }

  if (first === '.pnpm') {
    const packageName = segments[3];
    if (!segments[1] || !packageName) {
      return null;
    }

    if (packageName.startsWith('@')) {
      const packageSubpath = segments[4];
      if (!packageSubpath) {
        return null;
      }
      return `${packageName}/${packageSubpath}`;
    }

    return packageName;
  }

  if (first.startsWith('@')) {
    const packageSubpath = segments[1];
    if (!packageSubpath) {
      return null;
    }
    return `${first}/${packageSubpath}`;
  }

  return first;
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
  if (comparison.warnings && comparison.warnings.length > 0) {
    lines.push('Warnings:');
    for (const warning of comparison.warnings) {
      lines.push(`  ${warning}`);
    }
    lines.push('');
  }
  appendTriageSection(lines, comparison);
  lines.push('');
  if (comparison.details?.onlyTypeScriptRust?.ts2305ByModuleAndExport?.length) {
    lines.push('Top ONLY_RUST TS2305 by module/export:');
    for (const entry of comparison.details.onlyTypeScriptRust.ts2305ByModuleAndExport.slice(0, 10)) {
      lines.push(`  ${entry.moduleSpecifier} :: ${entry.exportName} [${entry.category}]  ${entry.count}`);
    }
    lines.push('');
  }
  if (comparison.details?.onlyTypeScriptRust?.ts2307ByModuleSpecifier?.length) {
    lines.push('Top ONLY_RUST TS2307 by module specifier:');
    for (const entry of comparison.details.onlyTypeScriptRust.ts2307ByModuleSpecifier.slice(0, 10)) {
      lines.push(`  ${entry.key} [${entry.category}]  ${entry.count}`);
    }
    lines.push('');
  }
  if (comparison.details?.onlyTypeScriptRust?.ts2304ByIdentifier?.length) {
    lines.push('Top ONLY_RUST TS2304 by identifier:');
    for (const entry of comparison.details.onlyTypeScriptRust.ts2304ByIdentifier.slice(0, 10)) {
      lines.push(`  ${entry.key} [${entry.category}]  ${entry.count}`);
    }
    lines.push('');
  }
  if (comparison.details?.onlyTypeScriptRust?.nodeModulesSourceDiagnosticsByPrefix?.length) {
    lines.push('Top ONLY_RUST node_modules source diagnostics by prefix:');
    for (const entry of comparison.details.onlyTypeScriptRust.nodeModulesSourceDiagnosticsByPrefix.slice(0, 10)) {
      lines.push(`  ${entry.key}  ${entry.count}`);
    }
    lines.push('');
  }
  if (comparison.details?.onlyTypeScriptRust?.nodeModulesJavaScriptSourceDiagnosticsByPrefix?.length) {
    lines.push('Top ONLY_RUST node_modules JavaScript source diagnostics by prefix:');
    for (const entry of comparison.details.onlyTypeScriptRust.nodeModulesJavaScriptSourceDiagnosticsByPrefix.slice(0, 10)) {
      lines.push(`  ${entry.key}  ${entry.count}`);
    }
    lines.push('');
  }
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

function buildComparisonWarnings(
  typescript: NormalizedDiagnostic[],
  typescriptRust: NormalizedDiagnostic[],
): string[] {
  const warnings: string[] = [];
  const rustDiagnosticsInNodeModules = typescriptRust.filter((diagnostic) =>
    diagnostic.fileName.includes('/node_modules/'),
  );
  const rustDiagnosticsInNodeModulesDeclarations = rustDiagnosticsInNodeModules.filter((diagnostic) =>
    diagnostic.fileName.endsWith('.d.ts') ||
    diagnostic.fileName.endsWith('.d.mts') ||
    diagnostic.fileName.endsWith('.d.cts'),
  );
  const rustDiagnosticsInNodeModulesSourceFiles = rustDiagnosticsInNodeModules.filter(
    (diagnostic) =>
      !diagnostic.fileName.endsWith('.d.ts') &&
      !diagnostic.fileName.endsWith('.d.mts') &&
      !diagnostic.fileName.endsWith('.d.cts'),
  );
  const rustDiagnosticsInNodeModulesJavaScriptSourceFiles = rustDiagnosticsInNodeModulesSourceFiles.filter(
    (diagnostic) =>
      diagnostic.fileName.endsWith('.js') ||
      diagnostic.fileName.endsWith('.jsx') ||
      diagnostic.fileName.endsWith('.mjs') ||
      diagnostic.fileName.endsWith('.cjs'),
  );
  const rustDiagnosticsInNodeModulesOtherSourceFiles = rustDiagnosticsInNodeModulesSourceFiles.filter(
    (diagnostic) =>
      !diagnostic.fileName.endsWith('.js') &&
      !diagnostic.fileName.endsWith('.jsx') &&
      !diagnostic.fileName.endsWith('.mjs') &&
      !diagnostic.fileName.endsWith('.cjs'),
  );
  const rustOnlyDiagnostics = typescriptRust.filter((diagnostic) =>
    diagnostic.code.startsWith('typescript-rust::'),
  );

  if (rustDiagnosticsInNodeModulesDeclarations.length > 0) {
    warnings.push(
      `Rust diagnostics from node_modules dependency declarations: ${rustDiagnosticsInNodeModulesDeclarations.length}`,
    );
  }

  if (rustDiagnosticsInNodeModulesSourceFiles.length > 0) {
    warnings.push(
      `Rust diagnostics from node_modules source files: ${rustDiagnosticsInNodeModulesSourceFiles.length}`,
    );
  }

  if (rustDiagnosticsInNodeModulesJavaScriptSourceFiles.length > 0) {
    warnings.push(
      `Rust diagnostics from node_modules JavaScript source files: ${rustDiagnosticsInNodeModulesJavaScriptSourceFiles.length}`,
    );
  }

  if (rustDiagnosticsInNodeModulesOtherSourceFiles.length > 0) {
    warnings.push(
      `Rust diagnostics from node_modules non-JavaScript source files: ${rustDiagnosticsInNodeModulesOtherSourceFiles.length}`,
    );
  }

  if (rustOnlyDiagnostics.length > 0) {
    warnings.push(
      `Rust-only typescript-rust::* diagnostics in tsc profile: ${rustOnlyDiagnostics.length}`,
    );
  }

  if (typescriptRust.length > typescript.length * 2) {
    warnings.push(
      `Severe over-report: typescript-rust diagnostics (${typescriptRust.length}) exceed TypeScript diagnostics (${typescript.length}) by more than 2x`,
    );
  }

  return warnings;
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
      '  --rustJobs <n>            Pass a deterministic project-checking job count to typescript-rust.',
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

export function readTsconfigPathsMappings(tsconfigPath: string): TsconfigPathMapping[] {
  let parsed: unknown;
  try {
    parsed = JSON.parse(readFileSync(tsconfigPath, 'utf8')) as unknown;
  } catch {
    return [];
  }

  const compilerOptions = (parsed as { compilerOptions?: unknown }).compilerOptions;
  if (!compilerOptions || typeof compilerOptions !== 'object') {
    return [];
  }

  const paths = (compilerOptions as { paths?: unknown }).paths;
  if (!paths || typeof paths !== 'object') {
    return [];
  }

  const mappings: TsconfigPathMapping[] = [];
  for (const [pattern, substitutions] of Object.entries(paths as Record<string, unknown>)) {
    if (typeof pattern !== 'string') {
      continue;
    }

    const entries = Array.isArray(substitutions)
      ? substitutions.filter((substitution): substitution is string => typeof substitution === 'string')
      : typeof substitutions === 'string'
        ? [substitutions]
        : [];

    if (entries.length === 0) {
      continue;
    }

    mappings.push({ pattern, substitutions: entries });
  }

  return mappings;
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
  const pathsMappings = mode.kind === 'project' ? readTsconfigPathsMappings(mode.resolvedTsconfig) : [];
  const tsc = runTsc(mode);
  const rust = runTypeScriptRust(mode, maxDiagnostics, mode.kind === 'project' ? mode.rustJobs : undefined);
  const rustOutput = rust.stdout.trim() ? rust.stdout : rust.stderr;

  const tscDiagnostics = limitDiagnostics(
    parseTypeScriptDiagnostics(`${tsc.stdout}${tsc.stderr}`, projectDir),
    maxDiagnostics,
  );
  const rustDiagnostics = limitDiagnostics(parseTypeScriptRustDiagnostics(rustOutput, projectDir), maxDiagnostics);

  return compareDiagnostics(
    mode.kind,
    comparisonDisplay,
    tscDiagnostics,
    rustDiagnostics,
    mode.ignoreConfig,
    mode.stubExternalModules,
    projectDir,
    pathsMappings,
    mode.kind === 'project' ? mode.rustJobs : undefined,
  );
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
