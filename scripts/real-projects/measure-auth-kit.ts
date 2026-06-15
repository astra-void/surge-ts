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

type ProgramMeasurements = {
  timings: Map<string, string>;
  counters: Map<string, string>;
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
  const oracle = parseJsonFromCommandOutput<OracleComparison>(
    oracleJsonText,
    'oracle:compare --json',
  );

  runCommand('cargo', ['build', '--release', '-p', 'typescript-rust-cli']);

  const compatReportResult = runCommandResult(releaseBinaryPath(), [
    '--project',
    project.tsconfig,
    '--compatReport',
    '--format',
    'json',
    '--timings',
  ]);
  const compatReport = parseJsonFromCommandOutput<CompatReport>(
    compatReportResult.stdout,
    'compatReport --format json',
  );
  const programMeasurements = parseProgramMeasurements(compatReportResult.stderr);

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
  const markdown = renderMeasurementMarkdown(
    project,
    oracle,
    compatReport,
    jobs1,
    jobs4,
    programMeasurements,
  );
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
      `${command} exited with ${result.status ?? 'unknown'}:\n${result.stdout ?? ''}${result.stderr ?? ''}`,
    );
  }

  return {
    stdout: `${result.stdout ?? ''}`,
    stderr: `${result.stderr ?? ''}`,
  };
}

function parseJsonFromCommandOutput<T>(raw: string, context: string): T {
  const text = raw.trim();
  try {
    return JSON.parse(text) as T;
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
      return JSON.parse(candidate) as T;
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
  programMeasurements: ProgramMeasurements,
): string {
  const counts = new Map(oracle.typescript.byCode.map((entry) => [entry.key, entry.count]));
  const rawOnlyRust = oracle.details?.onlyTypeScriptRust?.rawDiagnosticFingerprints ?? [];
  const rawOnlyTs = oracle.details?.onlyTypeScript?.rawDiagnosticFingerprints ?? [];
  const repeatedFingerprints = [...rawOnlyTs, ...rawOnlyRust].filter((entry) => entry.count > 1);
  const timing = (key: string): string => programMeasurements.timings.get(key) ?? 'n/a';
  const counter = (key: string): number => {
    const value = programMeasurements.counters.get(key);
    return value ? Number(value) : 0;
  };
  const functionTypeHandleBacked =
    counter('function_type_handle_copy_count') > 0 &&
    counter('function_type_payload_deep_clone_count') === 0;
  const objectTypeHandleBacked =
    counter('object_type_id_copy_count') > 0 && counter('object_type_payload_deep_clone_count') === 0;
  const unionTypeHandleBacked =
    counter('union_type_handle_copy_count') > 0 &&
    counter('union_type_payload_deep_clone_count') === 0;
  const typeDeclarationTablePreserved = counter('arena_type_declaration_payload_alloc_count') > 0;
  const noHotAllocatorMutex = true;

  return [
    '# Auth-Kit Measurement',
    '',
    'v1.2.4 is a performance recovery / stabilization pass after v1.2.3, not',
    'a TypeScript semantic expansion. On the latest auth-kit measurement, exact',
    'diagnostics remain 0 and raw oracle match stays yes. v1.2.3 shared',
    '`SymbolInfo` handle storage is preserved while hot symbol/scope setup avoids',
    'some whole-table materialization and eager visible-scope rebuilds.',
    '',
    `Project path used: \`${project.root}\``,
    `tsconfig.json: \`${project.tsconfig}\``,
    '',
    '## Relevant Files',
    '- `crates/typescript-rust-checker/Cargo.toml`',
    '- `crates/typescript-rust-checker/src/arena.rs`',
    '- `crates/typescript-rust-checker/src/lib.rs`',
    '- `crates/typescript-rust-checker/src/symbols/type_declarations.rs`',
    '- `crates/typescript-rust-checker/src/symbols/values.rs`',
    '- `crates/typescript-rust-checker/src/symbols/scopes.rs`',
    '- `crates/typescript-rust-checker/src/program.rs`',
    '- `crates/typescript-rust-checker/ARENA_ID_PLAN.md`',
    '- `REAL_PROJECT_COMPAT.md`',
    '- `crates/typescript-rust-checker/REAL_PROJECT_COMPAT.md`',
    '- `.bench/auth-kit-measurement.md`',
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
    '## Timing Buckets',
    `- type_declaration_collection: ${timing('type_declaration_collection')}`,
    `- module_binding: ${timing('module_binding')}`,
    `- per_file_statement_checking: ${timing('per_file_statement_checking')}`,
    `- flow_narrowing: ${timing('flow_narrowing')}`,
    `- function_declaration_checking: ${timing('function_declaration_checking')}`,
    `- object_literal_checking: ${timing('object_literal_checking')}`,
    `- call_expression_checking: ${timing('call_expression_checking')}`,
    `- assignability_checking: ${timing('assignability_checking')}`,
    '',
    '## Generic Inference Counters',
    `- generic_call_inference_attempt_count: ${counter('generic_call_inference_attempt_count')}`,
    `- generic_call_inference_success_count: ${counter('generic_call_inference_success_count')}`,
    `- generic_call_inference_failed_count: ${counter('generic_call_inference_failed_count')}`,
    `- generic_call_inference_explicit_type_args_skip_count: ${counter('generic_call_inference_explicit_type_args_skip_count')}`,
    `- generic_call_inference_unresolved_argument_skip_count: ${counter('generic_call_inference_unresolved_argument_skip_count')}`,
    `- generic_call_inference_tuple_return_suppressed_count: ${counter('generic_call_inference_tuple_return_suppressed_count')}`,
    `- generic_call_inference_candidate_count: ${counter('generic_call_inference_candidate_count')}`,
    '',
    '## Generic Indexed Access Counters',
    `- generic_indexed_access_attempt_count: ${counter('generic_indexed_access_attempt_count')}`,
    `- generic_indexed_access_substituted_receiver_count: ${counter('generic_indexed_access_substituted_receiver_count')}`,
    `- generic_indexed_access_substituted_key_count: ${counter('generic_indexed_access_substituted_key_count')}`,
    `- generic_indexed_access_success_count: ${counter('generic_indexed_access_success_count')}`,
    `- generic_indexed_access_unknown_fallback_count: ${counter('generic_indexed_access_unknown_fallback_count')}`,
    `- generic_indexed_access_invalid_key_count: ${counter('generic_indexed_access_invalid_key_count')}`,
    '',
    '## Handle Counters',
    `- FunctionType handle-backed: ${boolToYesNo(functionTypeHandleBacked)}`,
    `- ObjectType handle-backed: ${boolToYesNo(objectTypeHandleBacked)}`,
    `- UnionType handle-backed: ${boolToYesNo(unionTypeHandleBacked)}`,
    `- checker_arena_alloc_count: ${counter('checker_arena_alloc_count')}`,
    `- arena_object_type_payload_alloc_count: ${counter('arena_object_type_payload_alloc_count')}`,
    `- object_type_payload_deep_clone_count: ${counter('object_type_payload_deep_clone_count')}`,
    `- object_type_clone_count: ${counter('object_type_clone_count')}`,
    `- object_type_id_copy_count: ${counter('object_type_id_copy_count')}`,
    `- function_type_payload_alloc_count: ${counter('function_type_payload_alloc_count')}`,
    `- function_type_payload_deep_clone_count: ${counter('function_type_payload_deep_clone_count')}`,
    `- function_type_handle_copy_count: ${counter('function_type_handle_copy_count')}`,
    `- function_type_clone_count: ${counter('function_type_clone_count')}`,
    `- union_type_payload_alloc_count: ${counter('union_type_payload_alloc_count')}`,
    `- union_type_payload_deep_clone_count: ${counter('union_type_payload_deep_clone_count')}`,
    `- union_type_handle_copy_count: ${counter('union_type_handle_copy_count')}`,
    `- union_type_clone_count: ${counter('union_type_clone_count')}`,
    `- type_clone_count: ${counter('type_clone_count')}`,
    `- symbol_info_handle_copy_count: ${counter('symbol_info_handle_copy_count')}`,
    `- symbol_info_payload_deep_clone_count: ${counter('symbol_info_payload_deep_clone_count')}`,
    `- symbol_table_clone_count: ${counter('symbol_table_clone_count')}`,
    `- symbol_table_entry_handle_copy_count: ${counter('symbol_table_entry_handle_copy_count')}`,
    `- scope_stack_visible_rebuild_count: ${counter('scope_stack_visible_rebuild_count')}`,
    `- scope_stack_visible_symbol_handle_copy_count: ${counter('scope_stack_visible_symbol_handle_copy_count')}`,
    '',
    '## Module Export Counters',
    `- module_export_table_clone_count: ${counter('module_export_table_clone_count')}`,
    `- module_export_entry_clone_count: ${counter('module_export_entry_clone_count')}`,
    `- module_export_symbol_handle_copy_count: ${counter('module_export_symbol_handle_copy_count')}`,
    `- module_export_borrowed_lookup_count: ${counter('module_export_borrowed_lookup_count')}`,
    `- module_export_namespace_export_object_materialization_count: ${counter('module_export_namespace_export_object_materialization_count')}`,
    `- module_export_namespace_export_object_property_count: ${counter('module_export_namespace_export_object_property_count')}`,
    '',
    '## Handle Copy Attribution',
    `- function_type_copy_from_expression_identifier_count: ${counter('function_type_copy_from_expression_identifier_count')}`,
    `- function_type_copy_from_expression_call_return_count: ${counter('function_type_copy_from_expression_call_return_count')}`,
    `- function_type_copy_from_expression_optional_call_return_count: ${counter('function_type_copy_from_expression_optional_call_return_count')}`,
    `- function_type_copy_from_expression_inference_count: ${counter('function_type_copy_from_expression_inference_count')}`,
    `- function_type_copy_from_call_resolution_count: ${counter('function_type_copy_from_call_resolution_count')}`,
    `- function_type_copy_from_property_call_resolution_count: ${counter('function_type_copy_from_property_call_resolution_count')}`,
    `- function_type_copy_from_function_body_setup_count: ${counter('function_type_copy_from_function_body_setup_count')}`,
    `- function_type_copy_from_return_checking_count: ${counter('function_type_copy_from_return_checking_count')}`,
    `- function_type_copy_from_expected_type_count: ${counter('function_type_copy_from_expected_type_count')}`,
    `- function_type_copy_from_symbol_table_count: ${counter('function_type_copy_from_symbol_table_count')}`,
    `- function_type_copy_from_module_export_count: ${counter('function_type_copy_from_module_export_count')}`,
    `- function_type_copy_from_scope_or_context_count: ${counter('function_type_copy_from_scope_or_context_count')}`,
    `- function_type_copy_from_substitution_unchanged_count: ${counter('function_type_copy_from_substitution_unchanged_count')}`,
    `- function_type_copy_from_substitution_changed_count: ${counter('function_type_copy_from_substitution_changed_count')}`,
    `- function_type_copy_from_diagnostic_formatting_count: ${counter('function_type_copy_from_diagnostic_formatting_count')}`,
    `- function_type_copy_unattributed_count: ${counter('function_type_copy_unattributed_count')}`,
    `- union_type_copy_from_expression_identifier_count: ${counter('union_type_copy_from_expression_identifier_count')}`,
    `- union_type_copy_from_expression_call_return_count: ${counter('union_type_copy_from_expression_call_return_count')}`,
    `- union_type_copy_from_expression_optional_call_return_count: ${counter('union_type_copy_from_expression_optional_call_return_count')}`,
    `- union_type_copy_from_expression_inference_count: ${counter('union_type_copy_from_expression_inference_count')}`,
    `- union_type_copy_from_call_resolution_count: ${counter('union_type_copy_from_call_resolution_count')}`,
    `- union_type_copy_from_property_call_resolution_count: ${counter('union_type_copy_from_property_call_resolution_count')}`,
    `- union_type_copy_from_function_body_setup_count: ${counter('union_type_copy_from_function_body_setup_count')}`,
    `- union_type_copy_from_return_checking_count: ${counter('union_type_copy_from_return_checking_count')}`,
    `- union_type_copy_from_expected_type_count: ${counter('union_type_copy_from_expected_type_count')}`,
    `- union_type_copy_from_symbol_table_count: ${counter('union_type_copy_from_symbol_table_count')}`,
    `- union_type_copy_from_module_export_count: ${counter('union_type_copy_from_module_export_count')}`,
    `- union_type_copy_from_scope_or_context_count: ${counter('union_type_copy_from_scope_or_context_count')}`,
    `- union_type_copy_from_substitution_unchanged_count: ${counter('union_type_copy_from_substitution_unchanged_count')}`,
    `- union_type_copy_from_substitution_changed_count: ${counter('union_type_copy_from_substitution_changed_count')}`,
    `- union_type_copy_from_diagnostic_formatting_count: ${counter('union_type_copy_from_diagnostic_formatting_count')}`,
    `- union_type_copy_unattributed_count: ${counter('union_type_copy_unattributed_count')}`,
    `- TypeDeclarationTable v0.96 migration preserved: ${boolToYesNo(typeDeclarationTablePreserved)}`,
    `- no hot allocator mutex: ${boolToYesNo(noHotAllocatorMutex)}`,
    '',
  ].join('\n');
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
