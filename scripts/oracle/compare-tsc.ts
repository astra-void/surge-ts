#!/usr/bin/env tsx

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, statSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export type Source = 'typescript' | 'surge-ts';

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
  surgeTs: number;
};

export type CountEntry = {
  key: string;
  count: number;
};

export type ModuleExportCountEntry = {
  moduleSpecifier: string;
  exportName: string;
  count: number;
};

export type DiagnosticFingerprintCountEntry = DiagnosticFingerprint & {
  count: number;
};

export type DiagnosticTotals = {
  total: number;
  byCode: CountEntry[];
  byFileCode: CountEntry[];
  byFileCodeLine: CountEntry[];
};

export type MessageMismatch = {
  fileName: string;
  code: string;
  line: number | null;
  column: number | null;
  typescript: string;
  surgeTs: string;
};

export type MessageParity = {
  comparedLocations: number;
  matches: number;
  mismatches: MessageMismatch[];
};

export type ComparisonResult = {
  mode: 'project' | 'file';
  project: string | null;
  file: string | null;
  ignoreConfig?: boolean;
  surgeTsOptions?: {
    stubExternalModules?: boolean;
    rustJobs?: number;
  };
  warnings?: string[];
  tooling: {
    typescriptVersion: string;
    typescriptCommand: string;
    surgeTsCommand: string;
    surgeTsJobs?: number;
  };
  typescript: DiagnosticTotals;
  surgeTs: DiagnosticTotals;
  matches: {
    byCode: CountBucket[];
    onlyTypeScript: CountBucket[];
    onlySurgeTs: CountBucket[];
    byFileCode: CountBucket[];
    onlyTypeScriptFileCode: CountBucket[];
    onlySurgeTsFileCode: CountBucket[];
    byFileCodeLine: CountBucket[];
    onlyTypeScriptFileCodeLine: CountBucket[];
    onlySurgeTsFileCodeLine: CountBucket[];
  };
  messageParity: MessageParity;
  summary: {
    byCodeMatch: boolean;
    byFileCodeMatch: boolean;
    byFileCodeLineMatch: boolean | null;
    messageMatch: boolean | null;
  };
  details?: {
    onlySurgeTs?: {
      rawDiagnosticFingerprints?: DiagnosticFingerprintCountEntry[];
      rawTs2305ModuleExports?: ModuleExportCountEntry[];
      rawTs2307ModuleSpecifiers?: CountEntry[];
      rawTs2304Identifiers?: CountEntry[];
    };
    onlyTypeScript?: {
      rawDiagnosticFingerprints?: DiagnosticFingerprintCountEntry[];
    };
  };
};

export type ParsedArgs = {
  projectInput?: string;
  fileInput?: string;
  json: boolean;
  failOnMismatch: boolean;
  strictMessages: boolean;
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
// The oracle reference is TypeScript 7.0 (the native compiler, pinned as the
// canonical `typescript` package). Set SURGE_ORACLE_TSC=6 to compare against
// the legacy JS compiler (pinned as the `typescript-6` alias, kept as a
// benchmark reference). Both emit identical
// `file(line,col): error TSxxxx: message` lines, so the parser is shared.
// Each package exposes a `tsc` bin and only one can own `.bin/tsc`, so both are
// invoked through their resolved package bin path rather than `pnpm exec tsc`.
const oracleTypeScript = process.env.SURGE_ORACLE_TSC === '6' ? 'typescript-6' : 'typescript';
const oracleTscBinPath = path.join(workspaceRoot, 'node_modules', oracleTypeScript, 'bin', 'tsc');
const pinnedTypeScriptVersion = readPinnedTypeScriptVersion();
const subprocessMaxBuffer = 50 * 1024 * 1024;

const surgeBinExt = process.platform === 'win32' ? '.exe' : '';
const defaultSurgeBin = path.join(workspaceRoot, 'target', 'debug', `surge${surgeBinExt}`);

let resolvedSurgeBin: string | null = null;

// Resolve the surge-ts CLI binary once per process. Previously every comparison
// shelled out to `cargo run`, which re-runs cargo's freshness check on each
// invocation (multiplied across the sweep's child processes). Building once and
// then executing the binary directly keeps the same freshness guarantee while
// dropping the per-fixture cargo overhead. SURGE_TS_BIN points at a prebuilt
// binary; SURGE_TS_SKIP_BUILD=1 skips the build (set by callers that already built).
export function resolveSurgeBin(): string {
  if (resolvedSurgeBin) {
    return resolvedSurgeBin;
  }

  const override = process.env.SURGE_TS_BIN;
  if (override) {
    if (!existsSync(override)) {
      throw new Error(`SURGE_TS_BIN points to a missing binary: ${override}`);
    }
    resolvedSurgeBin = override;
    return override;
  }

  if (process.env.SURGE_TS_SKIP_BUILD !== '1') {
    const build = spawnSync('cargo', ['build', '-q', '-p', 'surge-ts-cli'], {
      cwd: workspaceRoot,
      encoding: 'utf8',
      maxBuffer: subprocessMaxBuffer,
    });
    if (build.error) {
      throw new Error(`failed to build surge-ts-cli: ${build.error.message}`);
    }
    if (build.status !== 0) {
      throw new Error(`failed to build surge-ts-cli:\n${build.stderr ?? ''}`);
    }
  }

  if (!existsSync(defaultSurgeBin)) {
    throw new Error(
      `surge-ts-cli binary not found at ${defaultSurgeBin}; run \`cargo build -p surge-ts-cli\``,
    );
  }
  resolvedSurgeBin = defaultSurgeBin;
  return defaultSurgeBin;
}

export const fixturePresets: Record<string, string> = {
  'declarations-basic': path.join(workspaceRoot, 'tests/compat-projects/declarations-basic/tsconfig.json'),
  'declarations-hardening': path.join(workspaceRoot, 'tests/compat-projects/declarations-hardening/tsconfig.json'),
  'module-export-visibility-hardening': path.join(workspaceRoot, 'tests/compat-projects/module-export-visibility-hardening/tsconfig.json'),
  'declaration-reexports-hardening': path.join(workspaceRoot, 'tests/compat-projects/declaration-reexports-hardening/tsconfig.json'),
  'namespace-import-reexport-basic': path.join(workspaceRoot, 'tests/compat-projects/namespace-import-reexport-basic/tsconfig.json'),
  'namespace-nested-member-lazy-scope-basic': path.join(workspaceRoot, 'tests/compat-projects/namespace-nested-member-lazy-scope-basic/tsconfig.json'),
  'function-type-binding-pattern-param-basic': path.join(workspaceRoot, 'tests/compat-projects/function-type-binding-pattern-param-basic/tsconfig.json'),
  'interface-extends-call-signature-basic': path.join(workspaceRoot, 'tests/compat-projects/interface-extends-call-signature-basic/tsconfig.json'),
  'package-exports-types-hardening': path.join(workspaceRoot, 'tests/compat-projects/package-exports-types-hardening/tsconfig.json'),
  'diagnostics-pack': path.join(workspaceRoot, 'tests/compat-projects/diagnostics-pack/tsconfig.json'),
  'generics-basic': path.join(workspaceRoot, 'tests/compat-projects/generics-basic/tsconfig.json'),
  'relative-js-extension-substitution-basic': path.join(workspaceRoot, 'tests/compat-projects/relative-js-extension-substitution-basic/tsconfig.json'),
  'relative-directory-index-basic': path.join(workspaceRoot, 'tests/compat-projects/relative-directory-index-basic/tsconfig.json'),
  'import-graph-generated-relative-basic': path.join(workspaceRoot, 'tests/compat-projects/import-graph-generated-relative-basic/tsconfig.json'),
  'paths-wildcard-import-graph-basic': path.join(workspaceRoot, 'tests/compat-projects/paths-wildcard-import-graph-basic/tsconfig.json'),
  'dependency-incomplete-declaration-export-fallback': path.join(workspaceRoot, 'tests/compat-projects/dependency-incomplete-declaration-export-fallback/tsconfig.json'),
  'generic-cache-unresolved-argument-diagnostics-basic': path.join(workspaceRoot, 'tests/compat-projects/generic-cache-unresolved-argument-diagnostics-basic/tsconfig.json'),
  'generic-cache-module-source-not-persisted-basic': path.join(workspaceRoot, 'tests/compat-projects/generic-cache-module-source-not-persisted-basic/tsconfig.json'),
  'generic-cache-dependency-instantiation-basic': path.join(workspaceRoot, 'tests/compat-projects/generic-cache-dependency-instantiation-basic/tsconfig.json'),
  'skip-lib-check-dependency-dts': path.join(workspaceRoot, 'tests/compat-projects/skip-lib-check-dependency-dts/tsconfig.json'),
  'skip-lib-check-local-dts': path.join(workspaceRoot, 'tests/compat-projects/skip-lib-check-local-dts/tsconfig.json'),
  'package-imports': path.join(workspaceRoot, 'tests/compat-projects/package-imports/tsconfig.json'),
  'module-forms': path.join(workspaceRoot, 'tests/compat-projects/module-forms/tsconfig.json'),
  'relative-deep': path.join(workspaceRoot, 'tests/compat-projects/relative-deep/tsconfig.json'),
  'private-types': path.join(workspaceRoot, 'tests/compat-projects/private-types/tsconfig.json'),
  'package-declarations': path.join(workspaceRoot, 'tests/compat-projects/package-declarations/tsconfig.json'),
  'builtin-visibility-project-graph-basic': path.join(workspaceRoot, 'tests/compat-projects/builtin-visibility-project-graph-basic/tsconfig.json'),
  'builtin-visibility-import-graph-basic': path.join(workspaceRoot, 'tests/compat-projects/builtin-visibility-import-graph-basic/tsconfig.json'),
  'builtin-visibility-function-body-basic': path.join(workspaceRoot, 'tests/compat-projects/builtin-visibility-function-body-basic/tsconfig.json'),
  'module-local-functions-basic': path.join(workspaceRoot, 'tests/compat-projects/module-local-functions-basic/tsconfig.json'),
  'function-body-local-visibility-basic': path.join(workspaceRoot, 'tests/compat-projects/function-body-local-visibility-basic/tsconfig.json'),
  'import-graph-dependency-js-not-source': path.join(workspaceRoot, 'tests/compat-projects/import-graph-dependency-js-not-source/tsconfig.json'),
  'parallel-ordering-basic': path.join(workspaceRoot, 'tests/compat-projects/parallel-ordering-basic/tsconfig.json'),
  'tsx-jsx-basic': path.join(workspaceRoot, 'tests/compat-projects/tsx-jsx-basic/tsconfig.json'),
  'tsx-jsx-expression-diagnostics-basic': path.join(workspaceRoot, 'tests/compat-projects/tsx-jsx-expression-diagnostics-basic/tsconfig.json'),
  'tsx-jsx-attributes-basic': path.join(workspaceRoot, 'tests/compat-projects/tsx-jsx-attributes-basic/tsconfig.json'),
  'tsx-generic-angle-regression-basic': path.join(workspaceRoot, 'tests/compat-projects/tsx-generic-angle-regression-basic/tsconfig.json'),
  'jsx-function-component-props-basic': path.join(workspaceRoot, 'tests/compat-projects/jsx-function-component-props-basic/tsconfig.json'),
  'jsx-intrinsic-elements-basic': path.join(workspaceRoot, 'tests/compat-projects/jsx-intrinsic-elements-basic/tsconfig.json'),
  'jsx-children-basic': path.join(workspaceRoot, 'tests/compat-projects/jsx-children-basic/tsconfig.json'),
  'jsx-component-member-tag-basic': path.join(workspaceRoot, 'tests/compat-projects/jsx-component-member-tag-basic/tsconfig.json'),
  'jsx-imported-component-props-basic': path.join(workspaceRoot, 'tests/compat-projects/jsx-imported-component-props-basic/tsconfig.json'),
  'jsx-dom-physical-lib-prop-basic': path.join(workspaceRoot, 'tests/compat-projects/jsx-dom-physical-lib-prop-basic/tsconfig.json'),
  'jsx-unresolved-no-cascade-basic': path.join(workspaceRoot, 'tests/compat-projects/jsx-unresolved-no-cascade-basic/tsconfig.json'),
  'jsx-runtime-module-namespace-basic': path.join(workspaceRoot, 'tests/compat-projects/jsx-runtime-module-namespace-basic/tsconfig.json'),
  'jsx-imported-alias-props-basic': path.join(workspaceRoot, 'tests/compat-projects/jsx-imported-alias-props-basic/tsconfig.json'),
  'auto-types-node-basic': path.join(workspaceRoot, 'tests/compat-projects/auto-types-node-basic/tsconfig.json'),
  'auto-types-disabled-empty-types-basic': path.join(workspaceRoot, 'tests/compat-projects/auto-types-disabled-empty-types-basic/tsconfig.json'),
  'auto-types-narrowed-types-basic': path.join(workspaceRoot, 'tests/compat-projects/auto-types-narrowed-types-basic/tsconfig.json'),
  'auto-types-scoped-basic': path.join(workspaceRoot, 'tests/compat-projects/auto-types-scoped-basic/tsconfig.json'),
  'auto-types-ancestor-visibility-basic': path.join(workspaceRoot, 'tests/compat-projects/auto-types-ancestor-visibility-basic/packages/app/tsconfig.json'),
  'auto-types-nearest-wins-basic': path.join(workspaceRoot, 'tests/compat-projects/auto-types-nearest-wins-basic/packages/app/tsconfig.json'),
  'type-roots-basic': path.join(workspaceRoot, 'tests/compat-projects/type-roots-basic/tsconfig.json'),
  'type-roots-ignore-default-node-modules-basic': path.join(workspaceRoot, 'tests/compat-projects/type-roots-ignore-default-node-modules-basic/tsconfig.json'),
  'type-roots-with-types-filter-basic': path.join(workspaceRoot, 'tests/compat-projects/type-roots-with-types-filter-basic/tsconfig.json'),
  'reference-types-node-basic': path.join(workspaceRoot, 'tests/compat-projects/reference-types-node-basic/tsconfig.json'),
  'reference-types-scoped-basic': path.join(workspaceRoot, 'tests/compat-projects/reference-types-scoped-basic/tsconfig.json'),
  'reference-types-recursive-basic': path.join(workspaceRoot, 'tests/compat-projects/reference-types-recursive-basic/tsconfig.json'),
  'reference-types-missing-basic': path.join(workspaceRoot, 'tests/compat-projects/reference-types-missing-basic/tsconfig.json'),
  'reference-types-relative-directive-basic': path.join(workspaceRoot, 'tests/compat-projects/reference-types-relative-directive-basic/tsconfig.json'),
  'dot-relative-specifier-basic': path.join(workspaceRoot, 'tests/compat-projects/dot-relative-specifier-basic/tsconfig.json'),
  'ambient-module-in-types-entry-basic': path.join(workspaceRoot, 'tests/compat-projects/ambient-module-in-types-entry-basic/tsconfig.json'),
  'reference-types-dependency-dts-basic': path.join(workspaceRoot, 'tests/compat-projects/reference-types-dependency-dts-basic/tsconfig.json'),
  'reference-types-missing-dependency-dts-skip-lib-check-basic': path.join(workspaceRoot, 'tests/compat-projects/reference-types-missing-dependency-dts-skip-lib-check-basic/tsconfig.json'),
  'reference-types-with-type-roots-basic': path.join(workspaceRoot, 'tests/compat-projects/reference-types-with-type-roots-basic/tsconfig.json'),
  'reference-types-dedupe-order-basic': path.join(workspaceRoot, 'tests/compat-projects/reference-types-dedupe-order-basic/tsconfig.json'),
  'node-protocol-buffer-basic': path.join(workspaceRoot, 'tests/compat-projects/node-protocol-buffer-basic/tsconfig.json'),
  'node-protocol-fs-basic': path.join(workspaceRoot, 'tests/compat-projects/node-protocol-fs-basic/tsconfig.json'),
  'node-protocol-type-only-basic': path.join(workspaceRoot, 'tests/compat-projects/node-protocol-type-only-basic/tsconfig.json'),
  'node-protocol-namespace-basic': path.join(workspaceRoot, 'tests/compat-projects/node-protocol-namespace-basic/tsconfig.json'),
  'node-protocol-no-node-types-basic': path.join(workspaceRoot, 'tests/compat-projects/node-protocol-no-node-types-basic/tsconfig.json'),
  'node-protocol-reference-types-node-basic': path.join(workspaceRoot, 'tests/compat-projects/node-protocol-reference-types-node-basic/tsconfig.json'),
  'node-protocol-types-empty-explicit-reference-basic': path.join(workspaceRoot, 'tests/compat-projects/node-protocol-types-empty-explicit-reference-basic/tsconfig.json'),
  'node-protocol-side-effect-import-basic': path.join(workspaceRoot, 'tests/compat-projects/node-protocol-side-effect-import-basic/tsconfig.json'),
  'interface-merging-basic': path.join(workspaceRoot, 'tests/compat-projects/interface-merging-basic/tsconfig.json'),
  'interface-merging-across-files-basic': path.join(workspaceRoot, 'tests/compat-projects/interface-merging-across-files-basic/tsconfig.json'),
  'interface-merging-conflict-basic': path.join(workspaceRoot, 'tests/compat-projects/interface-merging-conflict-basic/tsconfig.json'),
  'declare-global-interface-basic': path.join(workspaceRoot, 'tests/compat-projects/declare-global-interface-basic/tsconfig.json'),
  'declare-global-window-physical-lib-basic': path.join(workspaceRoot, 'tests/compat-projects/declare-global-window-physical-lib-basic/tsconfig.json'),
  'module-augmentation-package-interface-basic': path.join(workspaceRoot, 'tests/compat-projects/module-augmentation-package-interface-basic/tsconfig.json'),
  'module-augmentation-add-export-basic': path.join(workspaceRoot, 'tests/compat-projects/module-augmentation-add-export-basic/tsconfig.json'),
  'ambient-module-reopen-merge-basic': path.join(workspaceRoot, 'tests/compat-projects/ambient-module-reopen-merge-basic/tsconfig.json'),
  'ambient-namespace-value-merge-basic': path.join(workspaceRoot, 'tests/compat-projects/ambient-namespace-value-merge-basic/tsconfig.json'),
  'ambient-global-namespace-value-merge-basic': path.join(workspaceRoot, 'tests/compat-projects/ambient-global-namespace-value-merge-basic/tsconfig.json'),
  'umd-global-module-reference-basic': path.join(workspaceRoot, 'tests/compat-projects/umd-global-module-reference-basic/tsconfig.json'),
  'module-augmentation-unresolved-no-cascade': path.join(workspaceRoot, 'tests/compat-projects/module-augmentation-unresolved-no-cascade/tsconfig.json'),
  'interface-method-merge-basic': path.join(workspaceRoot, 'tests/compat-projects/interface-method-merge-basic/tsconfig.json'),
  'class-interface-merge-policy-pinned': path.join(workspaceRoot, 'tests/compat-projects/class-interface-merge-policy-pinned/tsconfig.json'),
  'physical-lib-iterator-for-of-basic': path.join(workspaceRoot, 'tests/compat-projects/physical-lib-iterator-for-of-basic/tsconfig.json'),
  'react19-jsx-function-component-basic': path.join(workspaceRoot, 'tests/compat-projects/react19-jsx-function-component-basic/tsconfig.json'),
  'react19-jsx-generic-component-basic': path.join(workspaceRoot, 'tests/compat-projects/react19-jsx-generic-component-basic/tsconfig.json'),
  'query-generics-observer-basic': path.join(workspaceRoot, 'tests/compat-projects/query-generics-observer-basic/tsconfig.json'),
  'query-generics-options-mapped-basic': path.join(workspaceRoot, 'tests/compat-projects/query-generics-options-mapped-basic/tsconfig.json'),
  'schema-inference-nested-basic': path.join(workspaceRoot, 'tests/compat-projects/schema-inference-nested-basic/tsconfig.json'),
  'schema-inference-recursive-basic': path.join(workspaceRoot, 'tests/compat-projects/schema-inference-recursive-basic/tsconfig.json'),
  'express-augmentation-cycle-basic': path.join(workspaceRoot, 'tests/compat-projects/express-augmentation-cycle-basic/tsconfig.json'),
  'express-augmentation-cycle-collision-pinned': path.join(workspaceRoot, 'tests/compat-projects/express-augmentation-cycle-collision-pinned/tsconfig.json'),
  'router-graph-procedures-basic': path.join(workspaceRoot, 'tests/compat-projects/router-graph-procedures-basic/tsconfig.json'),
  'router-graph-subscription-basic': path.join(workspaceRoot, 'tests/compat-projects/router-graph-subscription-basic/tsconfig.json'),
  'node-decl-callable-namespace-basic': path.join(workspaceRoot, 'tests/compat-projects/node-decl-callable-namespace-basic/tsconfig.json'),
  'node-decl-subpath-cts-mts-basic': path.join(workspaceRoot, 'tests/compat-projects/node-decl-subpath-cts-mts-basic/tsconfig.json'),
  'combined-conditional-mapped-indexed-basic': path.join(workspaceRoot, 'tests/compat-projects/combined-conditional-mapped-indexed-basic/tsconfig.json'),
  'combined-augmentation-generic-registry-basic': path.join(workspaceRoot, 'tests/compat-projects/combined-augmentation-generic-registry-basic/tsconfig.json'),
  'namespace-import-qualified-member-basic': path.join(workspaceRoot, 'tests/compat-projects/namespace-import-qualified-member-basic/tsconfig.json'),
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
  const hasMessageMismatch = comparison.summary.messageMatch === false;
  if ((args.failOnMismatch && hasMismatch) || (args.strictMessages && hasMessageMismatch)) {
    process.exitCode = 1;
  }
}

export function parseArgs(argv: string[]): ParsedArgs {
  const parsed: ParsedArgs = {
    json: false,
    failOnMismatch: false,
    strictMessages: false,
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
    } else if (arg === '--strictMessages') {
      parsed.strictMessages = true;
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
      throw new Error('--ignoreConfig is only supported with --file in the oracle.');
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
    const resolvedPath = resolveWorkspacePath(projectInput);
    if (existsSync(resolvedPath)) {
      const stats = statSync(resolvedPath);
      if (stats.isDirectory()) {
        const tsconfigPath = path.join(resolvedPath, 'tsconfig.json');
        if (existsSync(tsconfigPath) && statSync(tsconfigPath).isFile()) {
          return tsconfigPath;
        }

        throw new Error(`missing tsconfig.json at ${normalizePathForDisplay(tsconfigPath)}`);
      }

      if (stats.isFile() && isTsConfigPath(resolvedPath)) {
        return resolvedPath;
      }
    }

    if (projectInput.endsWith('.json')) {
      if (path.basename(projectInput).toLowerCase().includes('tsconfig')) {
        throw new Error(`missing tsconfig.json at ${normalizePathForDisplay(resolvedPath)}`);
      }

      throw new Error(
        `--project expects a preset name or tsconfig.json path. For single files, use --file ${projectInput}.`,
      );
    }

    const tsconfigPath = path.join(resolvedPath, 'tsconfig.json');
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
    throw new Error(`--file currently supports .ts source files only. Received ${fileInput}.`);
  }

  const resolvedFile = resolveWorkspacePath(fileInput);
  if (!existsSync(resolvedFile) || !statSync(resolvedFile).isFile()) {
    throw new Error(`missing TypeScript source file: ${normalizePathForDisplay(resolvedFile)}`);
  }

  return resolvedFile;
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
      ? [oracleTscBinPath, '--noEmit', '--pretty', 'false', '--project', mode.resolvedTsconfig]
      : [oracleTscBinPath, '--noEmit', '--pretty', 'false', mode.resolvedFile];
  if (mode.ignoreConfig) {
      args.splice(args.length - 1, 0, '--ignoreConfig');
  }
  const result = spawnSync(process.execPath, args, {
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

export function runSurgeTs(
  mode: OracleMode,
  maxDiagnostics?: number,
  rustJobs?: number,
): RunResult {
  const exePath = resolveSurgeBin();
  const args: string[] = [];

  // Argument order mirrors buildSurgeTsCommand so the printed command
  // matches what actually runs. The positional source file must come last.
  if (mode.kind === 'project') {
    args.push('--project', mode.resolvedTsconfig, '--format', 'json');
    if (mode.stubExternalModules) {
      args.push('--stubExternalModules');
    }
    if (rustJobs !== undefined) {
      args.push('--jobs', String(rustJobs));
    }
    if (maxDiagnostics !== undefined) {
      args.push('--maxDiagnostics', String(maxDiagnostics));
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
  }

  const result = spawnSync(exePath, args, {
    cwd: workspaceRoot,
    encoding: 'utf8',
    maxBuffer: subprocessMaxBuffer,
  });

  if (result.error) {
    throw new Error(`failed to run surge-ts-cli: ${result.error.message}`);
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

export function parseSurgeTsDiagnostics(
  output: string,
  projectDir: string,
): NormalizedDiagnostic[] {
  let parsed: unknown;
  try {
    parsed = JSON.parse(output);
  } catch (error) {
    throw new Error(
      `surge-ts-cli did not emit valid JSON diagnostics.\n${formatParseFailure(output, error)}`,
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
      source: 'surge-ts',
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

export function compareDiagnostics(
  mode: 'project' | 'file',
  targetDisplay: string,
  typescript: NormalizedDiagnostic[],
  surgeTs: NormalizedDiagnostic[],
  ignoreConfig?: boolean,
  stubExternalModules?: boolean,
  rustJobs?: number,
): ComparisonResult {
  const byCode = compareBuckets(typescript, surgeTs, keyByCode);
  const byFileCode = compareBuckets(typescript, surgeTs, keyByFileCode);
  const byFileCodeLine = compareBuckets(
    typescript.filter(hasLineInfo),
    surgeTs.filter(hasLineInfo),
    keyByFileCodeLine,
  );
  const onlyDiagnostics = subtractDiagnosticsByKey(
    typescript,
    surgeTs,
    keyByDiagnosticFingerprint,
  );
  const onlyTypeScriptDiagnostics = onlyDiagnostics.onlyLeft;
  const onlySurgeTsDiagnostics = onlyDiagnostics.onlyRight;
  const messageParity = compareMessages(typescript, surgeTs);

  return {
    mode,
    project: mode === 'project' ? targetDisplay : null,
    file: mode === 'file' ? targetDisplay : null,
    ignoreConfig: ignoreConfig ?? false,
    surgeTsOptions: {
      stubExternalModules: stubExternalModules ?? false,
      rustJobs,
    },
    warnings: buildComparisonWarnings(typescript, surgeTs),
    tooling: {
      typescriptVersion: pinnedTypeScriptVersion,
      typescriptCommand: buildTypeScriptCommand(mode, targetDisplay, ignoreConfig),
      surgeTsCommand: buildSurgeTsCommand(
        mode,
        targetDisplay,
        ignoreConfig,
        stubExternalModules,
        rustJobs,
      ),
      surgeTsJobs: rustJobs,
    },
    typescript: summarizeDiagnostics(typescript),
    surgeTs: summarizeDiagnostics(surgeTs),
    matches: {
      byCode: byCode.matches,
      onlyTypeScript: byCode.onlyTypeScript,
      onlySurgeTs: byCode.onlySurgeTs,
      byFileCode: byFileCode.matches,
      onlyTypeScriptFileCode: byFileCode.onlyTypeScript,
      onlySurgeTsFileCode: byFileCode.onlySurgeTs,
      byFileCodeLine: byFileCodeLine.matches,
      onlyTypeScriptFileCodeLine: byFileCodeLine.onlyTypeScript,
      onlySurgeTsFileCodeLine: byFileCodeLine.onlySurgeTs,
    },
    messageParity,
    summary: {
      byCodeMatch: byCode.onlyTypeScript.length === 0 && byCode.onlySurgeTs.length === 0,
      byFileCodeMatch:
        byFileCode.onlyTypeScript.length === 0 && byFileCode.onlySurgeTs.length === 0,
      byFileCodeLineMatch:
        byFileCodeLine.matches.length > 0 ||
        byFileCodeLine.onlyTypeScript.length > 0 ||
        byFileCodeLine.onlySurgeTs.length > 0
          ? byFileCodeLine.onlyTypeScript.length === 0 &&
            byFileCodeLine.onlySurgeTs.length === 0
          : null,
      messageMatch:
        messageParity.comparedLocations === 0 ? null : messageParity.mismatches.length === 0,
    },
    details: {
      onlyTypeScript: {
        rawDiagnosticFingerprints: groupDiagnosticsByFingerprint(onlyTypeScriptDiagnostics),
      },
      onlySurgeTs: {
        rawDiagnosticFingerprints: groupDiagnosticsByFingerprint(onlySurgeTsDiagnostics),
        rawTs2305ModuleExports: groupDiagnosticsByModuleExportExtractor(
          onlySurgeTsDiagnostics.filter((diagnostic) => diagnostic.code === 'TS2305'),
          (diagnostic) => {
            const exportInfo = extractTs2305ModuleExport(diagnostic.message);
            return exportInfo ? { moduleSpecifier: exportInfo.moduleSpecifier, exportName: exportInfo.exportName } : null;
          },
        ),
        rawTs2307ModuleSpecifiers: groupDiagnosticsByExtractor(
          onlySurgeTsDiagnostics.filter((diagnostic) => diagnostic.code === 'TS2307'),
          (diagnostic) => extractTs2307ModuleSpecifier(diagnostic.message),
        ),
        rawTs2304Identifiers: groupDiagnosticsByExtractor(
          onlySurgeTsDiagnostics.filter((diagnostic) => diagnostic.code === 'TS2304'),
          (diagnostic) => extractTs2304Identifier(diagnostic.message),
        ),
      },
    },
  };
}

/**
 * Pairs diagnostics that share an exact location and code (fileName, code,
 * line, column) and reports where only the message text differs. Span-level
 * differences are deliberately left to the byFileCodeLine dimension: a pair is
 * only message-compared when its location matches exactly, so the output
 * isolates pure message-text drift (literal vs widened types, alias names,
 * quoting) from span defects.
 */
export function compareMessages(
  typescript: NormalizedDiagnostic[],
  surgeTs: NormalizedDiagnostic[],
): MessageParity {
  const typeScriptByLocation = groupByLocation(typescript);
  const surgeTsByLocation = groupByLocation(surgeTs);

  let comparedLocations = 0;
  let matches = 0;
  const mismatches: MessageMismatch[] = [];

  const sharedKeys = [...typeScriptByLocation.keys()]
    .filter((key) => surgeTsByLocation.has(key))
    .sort((leftKey, rightKey) => leftKey.localeCompare(rightKey));

  for (const key of sharedKeys) {
    comparedLocations += 1;
    const typeScriptList = typeScriptByLocation.get(key) ?? [];
    const surgeTsList = surgeTsByLocation.get(key) ?? [];

    const surgeTsRemaining = new Map<string, number>();
    for (const diagnostic of surgeTsList) {
      const message = diagnostic.message ?? '';
      surgeTsRemaining.set(message, (surgeTsRemaining.get(message) ?? 0) + 1);
    }

    // Consume exact message matches first; whatever is left on each side is a
    // genuine text difference at the same location.
    const typeScriptUnmatched: string[] = [];
    for (const diagnostic of typeScriptList) {
      const message = diagnostic.message ?? '';
      const remaining = surgeTsRemaining.get(message) ?? 0;
      if (remaining > 0) {
        surgeTsRemaining.set(message, remaining - 1);
        matches += 1;
      } else {
        typeScriptUnmatched.push(message);
      }
    }

    const surgeTsUnmatched: string[] = [];
    for (const [message, count] of surgeTsRemaining) {
      for (let index = 0; index < count; index += 1) {
        surgeTsUnmatched.push(message);
      }
    }

    typeScriptUnmatched.sort((left, right) => left.localeCompare(right));
    surgeTsUnmatched.sort((left, right) => left.localeCompare(right));

    const representative = typeScriptList[0] ?? surgeTsList[0];
    const pairCount = Math.min(typeScriptUnmatched.length, surgeTsUnmatched.length);
    for (let index = 0; index < pairCount; index += 1) {
      mismatches.push({
        fileName: representative.fileName,
        code: representative.code,
        line: representative.line ?? null,
        column: representative.column ?? null,
        typescript: typeScriptUnmatched[index],
        surgeTs: surgeTsUnmatched[index],
      });
    }
  }

  mismatches.sort(
    (left, right) =>
      left.fileName.localeCompare(right.fileName) ||
      (left.line ?? -1) - (right.line ?? -1) ||
      (left.column ?? -1) - (right.column ?? -1) ||
      left.code.localeCompare(right.code),
  );

  return { comparedLocations, matches, mismatches };
}

function groupByLocation(diagnostics: NormalizedDiagnostic[]): Map<string, NormalizedDiagnostic[]> {
  const byLocation = new Map<string, NormalizedDiagnostic[]>();
  for (const diagnostic of diagnostics) {
    const key = keyByFullLocation(diagnostic);
    const existing = byLocation.get(key);
    if (existing) {
      existing.push(diagnostic);
    } else {
      byLocation.set(key, [diagnostic]);
    }
  }

  return byLocation;
}

export function keyByFullLocation(diagnostic: NormalizedDiagnostic): string {
  return `${diagnostic.fileName} :: ${diagnostic.code} :: line=${diagnostic.line ?? 'n/a'} :: column=${diagnostic.column ?? 'n/a'}`;
}

export function buildTypeScriptCommand(mode: 'project' | 'file', targetDisplay: string, ignoreConfig?: boolean): string {
  const bin = `node node_modules/${oracleTypeScript}/bin/tsc`;
  if (mode === 'project') {
    return `${bin} --noEmit --pretty false --project ${targetDisplay}`;
  }

  return ignoreConfig ? `${bin} --noEmit --pretty false --ignoreConfig ${targetDisplay}` : `${bin} --noEmit --pretty false ${targetDisplay}`;
}

export function buildSurgeTsCommand(
  mode: 'project' | 'file',
  targetDisplay: string,
  ignoreConfig?: boolean,
  stubExternalModules?: boolean,
  rustJobs?: number,
): string {
  const cargoToml = normalizePathForDisplay(path.join(workspaceRoot, 'Cargo.toml'));
  let args = `cargo run -q --manifest-path ${cargoToml} -p surge-ts-cli --`;

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
  onlySurgeTs: CountBucket[];
} {
  const leftCounts = countDiagnostics(left, keyFn);
  const rightCounts = countDiagnostics(right, keyFn);
  const keys = new Set([...leftCounts.keys(), ...rightCounts.keys()]);
  const sortedKeys = [...keys].sort((leftKey, rightKey) => leftKey.localeCompare(rightKey));
  const matches: CountBucket[] = [];
  const onlyTypeScript: CountBucket[] = [];
  const onlySurgeTs: CountBucket[] = [];

  for (const key of sortedKeys) {
    const leftCount = leftCounts.get(key) ?? 0;
    const rightCount = rightCounts.get(key) ?? 0;
    if (leftCount === rightCount) {
      if (leftCount > 0) {
        matches.push({ key, typescript: leftCount, surgeTs: rightCount });
      }
      continue;
    }

    if (leftCount > 0 && rightCount === 0) {
      onlyTypeScript.push({ key, typescript: leftCount, surgeTs: 0 });
      continue;
    }

    if (rightCount > 0 && leftCount === 0) {
      onlySurgeTs.push({ key, typescript: 0, surgeTs: rightCount });
      continue;
    }

    if (leftCount > rightCount) {
      onlyTypeScript.push({ key, typescript: leftCount, surgeTs: rightCount });
    } else {
      onlySurgeTs.push({ key, typescript: leftCount, surgeTs: rightCount });
    }
  }

  return { matches, onlyTypeScript, onlySurgeTs };
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
  extractor: (diagnostic: NormalizedDiagnostic) => { moduleSpecifier: string; exportName: string } | null,
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
      right.count - left.count || left.moduleSpecifier.localeCompare(right.moduleSpecifier) || left.exportName.localeCompare(right.exportName),
  );
}

export function groupDiagnosticsByFingerprint(
  diagnostics: NormalizedDiagnostic[],
): DiagnosticFingerprintCountEntry[] {
  const counts = new Map<string, DiagnosticFingerprintCountEntry>();

  for (const diagnostic of diagnostics) {
    const fingerprint = normalizeDiagnostic(diagnostic);
    const dedupeKey = keyByDiagnosticFingerprint(diagnostic);
    const existing = counts.get(dedupeKey);
    if (existing) {
      existing.count += 1;
      continue;
    }

    counts.set(dedupeKey, { ...fingerprint, count: 1 });
  }

  return [...counts.values()].sort(
    (left, right) =>
      right.count - left.count ||
      left.fileName.localeCompare(right.fileName) ||
      (left.line ?? -1) - (right.line ?? -1) ||
      (left.column ?? -1) - (right.column ?? -1) ||
      left.code.localeCompare(right.code) ||
      (left.message ?? '').localeCompare(right.message ?? ''),
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

export function extractTs2304Identifier(message?: string): string | null {
  if (!message) {
    return null;
  }

  const match = message.match(/Cannot find (?:name|namespace) ['"]([^'"]+)['"]/i);
  return match ? match[1] : null;
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

export function keyByDiagnosticFingerprint(diagnostic: NormalizedDiagnostic): string {
  return JSON.stringify(normalizeDiagnostic(diagnostic));
}

export function hasLineInfo(diagnostic: NormalizedDiagnostic): boolean {
  return typeof diagnostic.line === 'number' && typeof diagnostic.column === 'number';
}

export function formatDiagnosticFingerprintEntry(entry: DiagnosticFingerprintCountEntry): string {
  const location = `${entry.fileName}:${entry.line ?? 'n/a'}:${entry.column ?? 'n/a'}`;
  const message = entry.message ?? '(no message)';
  return `${location} ${entry.code} ${entry.count} ${message}`;
}

export function renderComparisonText(comparison: ComparisonResult): string {
  const lines: string[] = [];
  lines.push('TypeScript oracle comparison');
  lines.push(`Mode: ${comparison.mode}`);
  lines.push(comparison.mode === 'project' ? `Project: ${comparison.project}` : `File: ${comparison.file}`);
  lines.push('');

  if (comparison.surgeTsOptions?.stubExternalModules) {
    lines.push('surge-ts options: --stubExternalModules');
    lines.push('Note: --stubExternalModules is a surge-ts-only compatibility mode.');
    lines.push('');
  }

  lines.push('Tooling:');
  lines.push(`TypeScript version: ${comparison.tooling.typescriptVersion}`);
  lines.push(`TypeScript command: ${comparison.tooling.typescriptCommand}`);
  lines.push(`surge-ts command: ${comparison.tooling.surgeTsCommand}`);
  if (comparison.tooling.surgeTsJobs !== undefined) {
    lines.push(`surge-ts jobs: ${comparison.tooling.surgeTsJobs}`);
  }
  lines.push('');
  lines.push('Totals:');
  lines.push(`TypeScript diagnostics: ${comparison.typescript.total}`);
  lines.push(`surge-ts diagnostics: ${comparison.surgeTs.total}`);
  lines.push('');
  if (comparison.warnings && comparison.warnings.length > 0) {
    lines.push('Warnings:');
    for (const warning of comparison.warnings) {
      lines.push(`  ${warning}`);
    }
    lines.push('');
  }
  lines.push('Summary:');
  lines.push(`  Code-count match: ${comparison.summary.byCodeMatch ? 'yes' : 'no'}`);
  lines.push(`  File/code match: ${comparison.summary.byFileCodeMatch ? 'yes' : 'no'}`);
  lines.push(
    `  File/code/line match: ${
      comparison.summary.byFileCodeLineMatch === null
        ? 'n/a'
        : comparison.summary.byFileCodeLineMatch
          ? 'yes'
          : 'no'
    }`,
  );
  lines.push(
    `  Message match: ${
      comparison.summary.messageMatch === null
        ? 'n/a'
        : comparison.summary.messageMatch
          ? 'yes'
          : 'no'
    }`,
  );
  lines.push('');
  appendMessageParitySection(lines, comparison.messageParity);
  lines.push('Raw message extraction, not root-cause classification:');
  if (comparison.details?.onlySurgeTs?.rawTs2305ModuleExports?.length) {
    lines.push('  TS2305 module/export:');
    for (const entry of comparison.details.onlySurgeTs.rawTs2305ModuleExports.slice(0, 10)) {
      lines.push(`    ${entry.moduleSpecifier} :: ${entry.exportName}  ${entry.count}`);
    }
  }
  if (comparison.details?.onlySurgeTs?.rawTs2307ModuleSpecifiers?.length) {
    lines.push('  TS2307 specifiers:');
    for (const entry of comparison.details.onlySurgeTs.rawTs2307ModuleSpecifiers.slice(0, 10)) {
      lines.push(`    ${entry.key}  ${entry.count}`);
    }
  }
  if (comparison.details?.onlySurgeTs?.rawTs2304Identifiers?.length) {
    lines.push('  TS2304 identifiers:');
    for (const entry of comparison.details.onlySurgeTs.rawTs2304Identifiers.slice(0, 10)) {
      lines.push(`    ${entry.key}  ${entry.count}`);
    }
  }
  lines.push('');
  if (comparison.details?.onlySurgeTs?.rawDiagnosticFingerprints?.length) {
    lines.push('Top ONLY_RUST raw diagnostic fingerprints:');
    for (const entry of comparison.details.onlySurgeTs.rawDiagnosticFingerprints.slice(0, 10)) {
      lines.push(`  ${formatDiagnosticFingerprintEntry(entry)}`);
    }
    lines.push('');
  }
  if (comparison.details?.onlyTypeScript?.rawDiagnosticFingerprints?.length) {
    lines.push('Top ONLY_TS raw diagnostic fingerprints:');
    for (const entry of comparison.details.onlyTypeScript.rawDiagnosticFingerprints.slice(0, 10)) {
      lines.push(`  ${formatDiagnosticFingerprintEntry(entry)}`);
    }
    lines.push('');
  }
  lines.push('By code:');
  appendBucketSection(
    lines,
    comparison.matches.byCode,
    comparison.matches.onlyTypeScript,
    comparison.matches.onlySurgeTs,
  );
  lines.push('');
  lines.push('By file/code:');
  appendBucketSection(
    lines,
    comparison.matches.byFileCode,
    comparison.matches.onlyTypeScriptFileCode,
    comparison.matches.onlySurgeTsFileCode,
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
      comparison.matches.onlySurgeTsFileCodeLine,
    );
  }
  return `${lines.join('\n')}\n`;
}

function buildComparisonWarnings(
  typescript: NormalizedDiagnostic[],
  surgeTs: NormalizedDiagnostic[],
): string[] {
  const warnings: string[] = [];
  const rustOnlyDiagnostics = surgeTs.filter((diagnostic) =>
    diagnostic.code.startsWith('surge::'),
  );

  if (rustOnlyDiagnostics.length > 0) {
    warnings.push(
      `Rust-only surge::* diagnostics in tsc profile: ${rustOnlyDiagnostics.length}`,
    );
  }

  if (surgeTs.length > typescript.length * 2) {
    warnings.push(
      `Severe over-report: surge-ts diagnostics (${surgeTs.length}) exceed TypeScript diagnostics (${typescript.length}) by more than 2x`,
    );
  }

  return warnings;
}

export function appendMessageParitySection(lines: string[], messageParity: MessageParity): void {
  lines.push('Message parity (same file/code/line/column, message text differs):');
  if (messageParity.comparedLocations === 0) {
    lines.push('  (no diagnostics share an exact location on both sides)');
    lines.push('');
    return;
  }

  lines.push(
    `  Compared locations: ${messageParity.comparedLocations}  matches: ${messageParity.matches}  mismatches: ${messageParity.mismatches.length}`,
  );
  for (const mismatch of messageParity.mismatches.slice(0, 20)) {
    const location = `${mismatch.fileName}:${mismatch.line ?? 'n/a'}:${mismatch.column ?? 'n/a'}`;
    lines.push(`  ${location} ${mismatch.code}`);
    lines.push(`    tsc : ${mismatch.typescript}`);
    lines.push(`    rust: ${mismatch.surgeTs}`);
  }
  if (messageParity.mismatches.length > 20) {
    lines.push(`  ... and ${messageParity.mismatches.length - 20} more`);
  }
  lines.push('');
}

export function appendBucketSection(
  lines: string[],
  matches: CountBucket[],
  onlyTypeScript: CountBucket[],
  onlySurgeTs: CountBucket[],
): void {
  if (matches.length === 0 && onlyTypeScript.length === 0 && onlySurgeTs.length === 0) {
    lines.push('  (none)');
    return;
  }

  for (const bucket of matches) {
    lines.push(`MATCH ${formatBucketKey(bucket.key)} ${bucket.typescript}`);
  }

  for (const bucket of onlyTypeScript) {
    if (bucket.surgeTs === 0) {
      lines.push(`ONLY_TS ${formatBucketKey(bucket.key)} ${bucket.typescript}`);
    } else {
      lines.push(
        `DIFF ${formatBucketKey(bucket.key)} TypeScript=${bucket.typescript} surge-ts=${bucket.surgeTs}`,
      );
    }
  }

  for (const bucket of onlySurgeTs) {
    if (bucket.typescript === 0) {
      lines.push(`ONLY_RUST ${formatBucketKey(bucket.key)} ${bucket.surgeTs}`);
    } else {
      lines.push(
        `DIFF ${formatBucketKey(bucket.key)} TypeScript=${bucket.typescript} surge-ts=${bucket.surgeTs}`,
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
      '  --strictMessages          Exit with code 1 when any same-location message text differs from tsc.',
      '  --rustJobs <n>            Pass a deterministic project-checking job count to surge-ts.',
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

  const deps = packageJson.devDependencies ?? {};
  const spec = deps[oracleTypeScript] ?? 'unknown';
  // devDependency specs are pnpm aliases (`npm:typescript@7.0.2`); surface just
  // the resolved version for display.
  return spec.replace(/^npm:typescript@/, '');
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
  const rust = runSurgeTs(mode, maxDiagnostics, mode.kind === 'project' ? mode.rustJobs : undefined);
  const rustOutput = rust.stdout.trim() ? rust.stdout : rust.stderr;

  const tscDiagnostics = limitDiagnostics(
    parseTypeScriptDiagnostics(`${tsc.stdout}${tsc.stderr}`, projectDir),
    maxDiagnostics,
  );
  const rustDiagnostics = limitDiagnostics(parseSurgeTsDiagnostics(rustOutput, projectDir), maxDiagnostics);

  return compareDiagnostics(
    mode.kind,
    comparisonDisplay,
    tscDiagnostics,
    rustDiagnostics,
    mode.ignoreConfig,
    mode.stubExternalModules,
    mode.kind === 'project' ? mode.rustJobs : undefined,
  );
}

export function displayComparisonTargetPath(targetPath: string): string {
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
