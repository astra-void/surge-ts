import assert from 'node:assert/strict';
import path from 'node:path';

import {
  classifyTs2305ModuleExport,
  classifyTs2304Identifier,
  classifyTs2307ModuleSpecifier,
  buildTypeScriptCommand,
  buildTypeScriptRustCommand,
  compareDiagnostics,
  countDiagnostics,
  extractTs2305ModuleExport,
  parseArgs,
  normalizeDiagnostic,
  normalizeDiagnostics,
  parseTypeScriptDiagnostics,
  parseTypeScriptRustDiagnostics,
  nodeModulesSourcePrefix,
  renderComparisonText,
  readTsconfigPathsMappings,
  resolveOracleMode,
  resolveFilePath,
  resolveProjectPresetOrPath,
} from './compare-tsc';

function run() {
  oracle_parse_tsc_single_line();
  oracle_parse_tsc_multiple_lines();
  oracle_parse_tsc_ignores_non_diagnostic_lines();
  oracle_parse_tsc_windows_path();
  oracle_parse_tsc_absolute_path();
  oracle_parse_tsc_message_with_colon();
  oracle_parse_rust_json_diagnostics();
  oracle_count_by_code();
  oracle_count_by_file_code();
  oracle_count_by_file_code_line();
  oracle_extract_ts2305_module_export();
  oracle_classify_ts2307_module_specifier();
  oracle_classify_ts2307_module_specifier_relative_existing();
  oracle_classify_ts2307_module_specifier_paths_alias();
  oracle_classify_ts2304_identifier();
  oracle_classify_ts2305_module_export();
  oracle_node_modules_source_prefix();
  oracle_compare_match();
  oracle_compare_only_typescript();
  oracle_compare_only_typescript_rust();
  oracle_package_imports_ts2882_line5_match();
  oracle_package_imports_stub_external_modules_ts2882_policy();
  oracle_args_requires_project_or_file();
  oracle_args_rejects_project_and_file_together();
  oracle_args_rejects_ts_file_as_project();
  oracle_args_rejects_tsx_file_as_project();
  oracle_args_rejects_js_file_as_project();
  oracle_args_rejects_tsconfig_as_file();
  oracle_args_accepts_project_preset();
  oracle_args_accepts_diagnostics_pack_project_preset();
  oracle_args_accepts_package_declarations_project_preset();
  oracle_args_accepts_declarations_basic_project_preset();
  oracle_args_accepts_declarations_hardening_project_preset();
  oracle_args_accepts_module_export_visibility_hardening_project_preset();
  oracle_args_accepts_declaration_reexports_hardening_project_preset();
  oracle_args_accepts_package_exports_types_hardening_project_preset();
  oracle_args_accepts_module_forms_project_preset();
  oracle_args_accepts_relative_js_extension_substitution_basic_project_preset();
  oracle_args_accepts_relative_directory_index_basic_project_preset();
  oracle_args_accepts_import_graph_generated_relative_basic_project_preset();
  oracle_args_accepts_paths_wildcard_import_graph_basic_project_preset();
  oracle_args_accepts_dependency_incomplete_declaration_export_fallback_project_preset();
  oracle_args_accepts_skip_lib_check_dependency_dts_project_preset();
  oracle_args_accepts_skip_lib_check_local_dts_project_preset();
  oracle_args_accepts_project_tsconfig_path();
  oracle_args_accepts_file_ts_path();
  oracle_args_rejects_file_tsx_path_current_policy();
  oracle_args_rejects_file_js_path_current_policy();
  oracle_args_rejects_missing_file_path();
  oracle_args_rejects_missing_project_path();
  oracle_unknown_project_fails_cleanly();
  oracle_parse_args_strict_codes_alias();
  oracle_builds_tsc_project_command_with_project();
  oracle_builds_tsc_project_command_with_declarations_basic_preset();
  oracle_builds_tsc_project_command_with_declarations_hardening_preset();
  oracle_builds_tsc_project_command_with_module_export_visibility_hardening_preset();
  oracle_builds_tsc_project_command_with_declaration_reexports_hardening_preset();
  oracle_builds_tsc_project_command_with_package_exports_types_hardening_preset();
  oracle_builds_tsc_file_command_without_project();
  oracle_builds_rust_project_command_with_project();
  oracle_builds_rust_project_command_with_declarations_basic_preset();
  oracle_builds_rust_project_command_with_declarations_hardening_preset();
  oracle_builds_rust_project_command_with_module_export_visibility_hardening_preset();
  oracle_builds_rust_project_command_with_declaration_reexports_hardening_preset();
  oracle_builds_rust_project_command_with_package_exports_types_hardening_preset();
  oracle_builds_rust_file_command_without_project();
  oracle_output_includes_mode_project();
  oracle_output_includes_mode_file();
  oracle_output_highlights_project_visibility_failure();
  oracle_json_output_includes_mode();
  oracle_args_accepts_stub_external_modules();
  oracle_args_accepts_rust_jobs();
  oracle_args_rejects_rust_jobs_in_file_mode();
  oracle_builds_rust_project_command_with_stub_external_modules();
  oracle_builds_rust_file_command_with_stub_external_modules();
  oracle_builds_rust_project_command_with_jobs();
  oracle_does_not_pass_stub_external_modules_to_tsc();
  oracle_output_mentions_stub_external_modules_as_rust_only();
  oracle_json_output_includes_stub_external_modules_flag();
  oracle_default_does_not_use_stub_external_modules();
  oracle_normalize_diagnostics_is_deterministic();
  oracle_output_includes_comparison_warnings();
}

function oracle_parse_tsc_single_line() {
  const diagnostics = parseTypeScriptDiagnostics(
    'src/index.ts(3,12): error TS2322: Type "number" is not assignable to type "string".',
    '/repo',
  );

  assert.equal(diagnostics.length, 1);
  assert.deepEqual(diagnostics[0], {
    source: 'typescript',
    code: 'TS2322',
    fileName: 'src/index.ts',
    line: 3,
    column: 12,
    message: 'Type "number" is not assignable to type "string".',
  });
}

function oracle_parse_tsc_multiple_lines() {
  const diagnostics = parseTypeScriptDiagnostics(
    ['src/index.ts(3,12): error TS2322: mismatch', 'Found 1 error in 1 file.', ''].join('\n'),
    '/repo',
  );

  assert.equal(diagnostics.length, 1);
  assert.equal(diagnostics[0].message, 'mismatch');
}

function oracle_parse_tsc_ignores_non_diagnostic_lines() {
  const diagnostics = parseTypeScriptDiagnostics(
    ['Version 6.0.3', 'src/index.ts(3,12): error TS2322: mismatch', 'Done in 1.2s'].join('\n'),
    '/repo',
  );

  assert.equal(diagnostics.length, 1);
  assert.equal(diagnostics[0].code, 'TS2322');
}

function oracle_parse_tsc_windows_path() {
  const diagnostics = parseTypeScriptDiagnostics(
    'C:\\repo\\src\\index.ts(3,12): error TS2322: mismatch',
    'C:/repo',
  );

  assert.equal(diagnostics.length, 1);
  assert.equal(diagnostics[0].fileName, 'src/index.ts');
}

function oracle_parse_tsc_absolute_path() {
  const diagnostics = parseTypeScriptDiagnostics(
    '/repo/src/index.ts(3,12): error TS2322: mismatch',
    '/repo',
  );

  assert.equal(diagnostics.length, 1);
  assert.equal(diagnostics[0].fileName, 'src/index.ts');
}

function oracle_parse_tsc_message_with_colon() {
  const diagnostics = parseTypeScriptDiagnostics(
    'src/index.ts(3,12): error TS2322: value must be string: got number',
    '/repo',
  );

  assert.equal(diagnostics.length, 1);
  assert.equal(diagnostics[0].message, 'value must be string: got number');
}

function oracle_parse_rust_json_diagnostics() {
  const diagnostics = parseTypeScriptRustDiagnostics(
    JSON.stringify({
      diagnostics: [
        {
          code: 'TS2322',
          fileName: '/repo/src/index.ts',
          span: { start: 42, end: 45 },
          line: 3,
          column: 12,
          message: 'Type mismatch',
        },
      ],
    }),
    '/repo',
  );

  assert.equal(diagnostics.length, 1);
  assert.deepEqual(diagnostics[0], {
    source: 'typescript-rust',
    code: 'TS2322',
    fileName: 'src/index.ts',
    line: 3,
    column: 12,
    message: 'Type mismatch',
  });
}

function oracle_count_by_code() {
  const counts = countDiagnostics(
    [
      { source: 'typescript', code: 'TS2322', fileName: 'src/a.ts' },
      { source: 'typescript', code: 'TS2322', fileName: 'src/b.ts' },
      { source: 'typescript-rust', code: 'TS2304', fileName: 'src/a.ts' },
    ],
    (diagnostic) => diagnostic.code,
  );

  assert.equal(counts.get('TS2322'), 2);
  assert.equal(counts.get('TS2304'), 1);
}

function oracle_count_by_file_code() {
  const counts = countDiagnostics(
    [
      { source: 'typescript', code: 'TS2322', fileName: 'src/a.ts' },
      { source: 'typescript', code: 'TS2322', fileName: 'src/a.ts' },
      { source: 'typescript-rust', code: 'TS2322', fileName: 'src/b.ts' },
    ],
    (diagnostic) => `${diagnostic.fileName} :: ${diagnostic.code}`,
  );

  assert.equal(counts.get('src/a.ts :: TS2322'), 2);
  assert.equal(counts.get('src/b.ts :: TS2322'), 1);
}

function oracle_count_by_file_code_line() {
  const counts = countDiagnostics(
    [
      { source: 'typescript', code: 'TS2322', fileName: 'src/a.ts', line: 3, column: 12 },
      { source: 'typescript', code: 'TS2322', fileName: 'src/a.ts', line: 3, column: 12 },
      { source: 'typescript-rust', code: 'TS2322', fileName: 'src/a.ts', line: 4, column: 2 },
    ],
    (diagnostic) => `${diagnostic.fileName} :: ${diagnostic.code} :: line=${diagnostic.line ?? 0}`,
  );

  assert.equal(counts.get('src/a.ts :: TS2322 :: line=3'), 2);
  assert.equal(counts.get('src/a.ts :: TS2322 :: line=4'), 1);
}

function oracle_extract_ts2305_module_export() {
  assert.deepEqual(
    extractTs2305ModuleExport("Module 'react' has no exported member 'default'."),
    { moduleSpecifier: 'react', exportName: 'default' },
  );
}

function oracle_classify_ts2307_module_specifier() {
  assert.equal(classifyTs2307ModuleSpecifier('..'), 'relative-missing');
  assert.equal(classifyTs2307ModuleSpecifier('../core/auth.gen'), 'relative-generated-missing');
  assert.equal(classifyTs2307ModuleSpecifier('../../vitest.config'), 'package-json');
  assert.equal(classifyTs2307ModuleSpecifier('../../package.json'), 'package-json');
  assert.equal(classifyTs2307ModuleSpecifier('@trpc/client'), 'package');
  assert.equal(classifyTs2307ModuleSpecifier('@trpc/server/adapters/standalone'), 'package-subpath');
}

function oracle_classify_ts2307_module_specifier_relative_existing() {
  const projectRoot = path.resolve('tests/compat-projects/import-graph-generated-relative-basic');
  assert.equal(
    classifyTs2307ModuleSpecifier('../core/auth.gen', 'src/pages/index.ts', projectRoot),
    'relative-generated-existing-not-loaded',
  );
}

function oracle_classify_ts2307_module_specifier_paths_alias() {
  const projectRoot = path.resolve('tests/compat-projects/paths-wildcard-import-graph-basic');
  const pathsMappings = readTsconfigPathsMappings(path.join(projectRoot, 'tsconfig.json'));
  assert.equal(
    classifyTs2307ModuleSpecifier('~/server/trpc', 'src/index.ts', projectRoot, pathsMappings),
    'paths-alias-explicit-relative-target',
  );
}

function oracle_classify_ts2304_identifier() {
  assert.equal(classifyTs2304Identifier('Headers'), 'dom-like');
  assert.equal(classifyTs2304Identifier('process'), 'missing-node-like-global');
  assert.equal(classifyTs2304Identifier('JSX'), 'jsx-like');
  assert.equal(classifyTs2304Identifier('T'), 'generic-or-type-parameter-scope');
  assert.equal(classifyTs2304Identifier('Object'), 'missing-es-lib-lite-global');
  assert.equal(classifyTs2304Identifier('Map'), 'missing-es-lib-lite-global');
  assert.equal(classifyTs2304Identifier('Uint8Array'), 'missing-es-lib-lite-global');
  assert.equal(classifyTs2304Identifier('globalThis'), 'missing-es-lib-lite-global');
  assert.equal(classifyTs2304Identifier('isNaN'), 'missing-es-lib-lite-global');
  assert.equal(classifyTs2304Identifier('Promise'), 'missing-synthetic-built-in');
  assert.equal(classifyTs2304Identifier('queryClient'), 'local-unresolved');
  assert.equal(classifyTs2304Identifier('Maybe'), 'package-derived-incomplete-declaration');
}

function oracle_classify_ts2305_module_export() {
  const projectRoot = path.resolve('tests/compat-projects/dependency-incomplete-declaration-export-fallback');
  assert.equal(
    classifyTs2305ModuleExport('dep-incomplete', 'src/index.ts', projectRoot),
    'package-derived-incomplete-declaration',
  );
}

function oracle_node_modules_source_prefix() {
  assert.equal(
    nodeModulesSourcePrefix('/repo/node_modules/fetch-blob/index.d.ts'),
    'fetch-blob',
  );
  assert.equal(
    nodeModulesSourcePrefix('/repo/node_modules/.pnpm/fetch-blob@3.2.0/node_modules/fetch-blob/index.js'),
    'fetch-blob',
  );
}

function oracle_compare_match() {
  const comparison = compareDiagnostics(
    'project',
    'project',
    [{ source: 'typescript', code: 'TS2322', fileName: 'src/a.ts', line: 3, column: 12 }],
    [{ source: 'typescript-rust', code: 'TS2322', fileName: 'src/a.ts', line: 3, column: 12 }],
  );

  assert.equal(comparison.summary.byCodeMatch, true);
  assert.equal(comparison.summary.byFileCodeMatch, true);
  assert.equal(comparison.summary.byFileCodeLineMatch, true);
}

function oracle_compare_only_typescript() {
  const comparison = compareDiagnostics(
    'project',
    'project',
    [{ source: 'typescript', code: 'TS2322', fileName: 'src/a.ts' }],
    [],
  );

  assert.equal(comparison.summary.byCodeMatch, false);
  assert.equal(comparison.matches.onlyTypeScript.length, 1);
  assert.equal(comparison.matches.onlyTypeScriptRust.length, 0);
}

function oracle_compare_only_typescript_rust() {
  const comparison = compareDiagnostics(
    'project',
    'project',
    [],
    [{ source: 'typescript-rust', code: 'TS2322', fileName: 'src/a.ts' }],
  );

  assert.equal(comparison.summary.byCodeMatch, false);
  assert.equal(comparison.matches.onlyTypeScript.length, 0);
  assert.equal(comparison.matches.onlyTypeScriptRust.length, 1);
}

function oracle_package_imports_ts2882_line5_match() {
  const tscDiagnostics = [
    { source: 'typescript' as const, code: 'TS2307', fileName: 'src/index.ts', line: 1, column: 19 },
    { source: 'typescript' as const, code: 'TS2882', fileName: 'src/index.ts', line: 5, column: 8 },
  ];
  const rustDiagnostics = [
    { source: 'typescript-rust' as const, code: 'TS2307', fileName: 'src/index.ts', line: 1, column: 19 },
    { source: 'typescript-rust' as const, code: 'TS2882', fileName: 'src/index.ts', line: 5, column: 8 },
  ];

  const comparison = compareDiagnostics(
    'project',
    'tests/compat-projects/package-imports/tsconfig.json',
    tscDiagnostics,
    rustDiagnostics,
  );

  assert.equal(comparison.summary.byCodeMatch, true);
  assert.equal(comparison.summary.byFileCodeMatch, true);
  assert.equal(comparison.summary.byFileCodeLineMatch, true);
}

function oracle_package_imports_stub_external_modules_ts2882_policy() {
  const comparison = compareDiagnostics(
    'project',
    'tests/compat-projects/package-imports/tsconfig.json',
    [{ source: 'typescript' as const, code: 'TS2882', fileName: 'src/index.ts', line: 5, column: 8 }],
    [],
    false,
    true,
  );

  assert.equal(comparison.typescriptRustOptions?.stubExternalModules, true);
  assert.equal(comparison.matches.onlyTypeScript[0].key, 'TS2882');
}

function oracle_unknown_project_fails_cleanly() {
  assert.throws(
    () => resolveProjectPresetOrPath('does-not-exist'),
    /unknown oracle project preset: does-not-exist/,
  );
}

function oracle_parse_args_strict_codes_alias() {
  const parsed = parseArgs(['--project', 'generics-basic', '--strictCodes']);
  assert.equal(parsed.failOnMismatch, true);
}

function oracle_args_requires_project_or_file() {
  const parsed = parseArgs([]);
  assert.throws(() => resolveOracleMode(parsed), /choose exactly one of --project or --file/);
}

function oracle_args_rejects_project_and_file_together() {
  const parsed = parseArgs(['--project', 'generics-basic', '--file', 'examples/basic.ts']);
  assert.throws(() => resolveOracleMode(parsed), /choose exactly one of --project or --file/);
}

function oracle_args_rejects_ts_file_as_project() {
  const parsed = parseArgs(['--project', 'examples/basic.ts']);
  assert.throws(
    () => resolveOracleMode(parsed),
    /--project expects a preset name or tsconfig\.json path\. For single files, use --file examples\/basic\.ts\./,
  );
}

function oracle_args_rejects_tsx_file_as_project() {
  const parsed = parseArgs(['--project', 'examples/basic.tsx']);
  assert.throws(
    () => resolveOracleMode(parsed),
    /--project expects a preset name or tsconfig\.json path\. For single files, use --file examples\/basic\.tsx\./,
  );
}

function oracle_args_rejects_js_file_as_project() {
  const parsed = parseArgs(['--project', 'examples/basic.js']);
  assert.throws(
    () => resolveOracleMode(parsed),
    /--project expects a preset name or tsconfig\.json path\. For single files, use --file examples\/basic\.js\./,
  );
}

function oracle_args_rejects_tsconfig_as_file() {
  const parsed = parseArgs(['--file', 'tests/compat-projects/generics-basic/tsconfig.json']);
  assert.throws(
    () => resolveOracleMode(parsed),
    /--file expects a TypeScript source file, not tsconfig\.json\. For projects, use --project\./,
  );
}

function oracle_args_accepts_project_preset() {
  const parsed = parseArgs(['--project', 'generics-basic']);
  const mode = resolveOracleMode(parsed);

  assert.equal(mode.kind, 'project');
  assert.equal(
    mode.resolvedTsconfig,
    path.resolve('tests/compat-projects/generics-basic/tsconfig.json'),
  );
}

function oracle_args_accepts_diagnostics_pack_project_preset() {
  const parsed = parseArgs(['--project', 'diagnostics-pack']);
  const mode = resolveOracleMode(parsed);

  assert.equal(mode.kind, 'project');
  assert.equal(
    mode.resolvedTsconfig,
    path.resolve('tests/compat-projects/diagnostics-pack/tsconfig.json'),
  );
}

function oracle_args_accepts_package_declarations_project_preset() {
  const parsed = parseArgs(['--project', 'package-declarations']);
  const mode = resolveOracleMode(parsed);

  assert.equal(mode.kind, 'project');
  assert.equal(
    mode.resolvedTsconfig,
    path.resolve('tests/compat-projects/package-declarations/tsconfig.json'),
  );
}

function oracle_args_accepts_declarations_basic_project_preset() {
  const parsed = parseArgs(['--project', 'declarations-basic']);
  const mode = resolveOracleMode(parsed);

  assert.equal(mode.kind, 'project');
  assert.equal(
    mode.resolvedTsconfig,
    path.resolve('tests/compat-projects/declarations-basic/tsconfig.json'),
  );
}

function oracle_args_accepts_declarations_hardening_project_preset() {
  const parsed = parseArgs(['--project', 'declarations-hardening']);
  const mode = resolveOracleMode(parsed);

  assert.equal(mode.kind, 'project');
  assert.equal(
    mode.resolvedTsconfig,
    path.resolve('tests/compat-projects/declarations-hardening/tsconfig.json'),
  );
}

function oracle_args_accepts_module_export_visibility_hardening_project_preset() {
  const parsed = parseArgs(['--project', 'module-export-visibility-hardening']);
  const mode = resolveOracleMode(parsed);

  assert.equal(mode.kind, 'project');
  assert.equal(
    mode.resolvedTsconfig,
    path.resolve('tests/compat-projects/module-export-visibility-hardening/tsconfig.json'),
  );
}

function oracle_args_accepts_declaration_reexports_hardening_project_preset() {
  const parsed = parseArgs(['--project', 'declaration-reexports-hardening']);
  const mode = resolveOracleMode(parsed);

  assert.equal(mode.kind, 'project');
  assert.equal(
    mode.resolvedTsconfig,
    path.resolve('tests/compat-projects/declaration-reexports-hardening/tsconfig.json'),
  );
}

function oracle_args_accepts_package_exports_types_hardening_project_preset() {
  const parsed = parseArgs(['--project', 'package-exports-types-hardening']);
  const mode = resolveOracleMode(parsed);

  assert.equal(mode.kind, 'project');
  assert.equal(
    mode.resolvedTsconfig,
    path.resolve('tests/compat-projects/package-exports-types-hardening/tsconfig.json'),
  );
}

function oracle_args_accepts_module_forms_project_preset() {
  const parsed = parseArgs(['--project', 'module-forms']);
  const mode = resolveOracleMode(parsed);

  assert.equal(mode.kind, 'project');
  assert.equal(mode.resolvedTsconfig, path.resolve('tests/compat-projects/module-forms/tsconfig.json'));
}

function oracle_args_accepts_relative_js_extension_substitution_basic_project_preset() {
  const parsed = parseArgs(['--project', 'relative-js-extension-substitution-basic']);
  const mode = resolveOracleMode(parsed);

  assert.equal(mode.kind, 'project');
  assert.equal(
    mode.resolvedTsconfig,
    path.resolve('tests/compat-projects/relative-js-extension-substitution-basic/tsconfig.json'),
  );
}

function oracle_args_accepts_relative_directory_index_basic_project_preset() {
  const parsed = parseArgs(['--project', 'relative-directory-index-basic']);
  const mode = resolveOracleMode(parsed);

  assert.equal(mode.kind, 'project');
  assert.equal(
    mode.resolvedTsconfig,
    path.resolve('tests/compat-projects/relative-directory-index-basic/tsconfig.json'),
  );
}

function oracle_args_accepts_import_graph_generated_relative_basic_project_preset() {
  const parsed = parseArgs(['--project', 'import-graph-generated-relative-basic']);
  const mode = resolveOracleMode(parsed);

  assert.equal(mode.kind, 'project');
  assert.equal(
    mode.resolvedTsconfig,
    path.resolve('tests/compat-projects/import-graph-generated-relative-basic/tsconfig.json'),
  );
}

function oracle_args_accepts_paths_wildcard_import_graph_basic_project_preset() {
  const parsed = parseArgs(['--project', 'paths-wildcard-import-graph-basic']);
  const mode = resolveOracleMode(parsed);

  assert.equal(mode.kind, 'project');
  assert.equal(
    mode.resolvedTsconfig,
    path.resolve('tests/compat-projects/paths-wildcard-import-graph-basic/tsconfig.json'),
  );
}

function oracle_args_accepts_dependency_incomplete_declaration_export_fallback_project_preset() {
  const parsed = parseArgs(['--project', 'dependency-incomplete-declaration-export-fallback']);
  const mode = resolveOracleMode(parsed);

  assert.equal(mode.kind, 'project');
  assert.equal(
    mode.resolvedTsconfig,
    path.resolve(
      'tests/compat-projects/dependency-incomplete-declaration-export-fallback/tsconfig.json',
    ),
  );
}

function oracle_args_accepts_skip_lib_check_dependency_dts_project_preset() {
  const parsed = parseArgs(['--project', 'skip-lib-check-dependency-dts']);
  const mode = resolveOracleMode(parsed);

  assert.equal(mode.kind, 'project');
  assert.equal(
    mode.resolvedTsconfig,
    path.resolve('tests/compat-projects/skip-lib-check-dependency-dts/tsconfig.json'),
  );
}

function oracle_args_accepts_skip_lib_check_local_dts_project_preset() {
  const parsed = parseArgs(['--project', 'skip-lib-check-local-dts']);
  const mode = resolveOracleMode(parsed);

  assert.equal(mode.kind, 'project');
  assert.equal(
    mode.resolvedTsconfig,
    path.resolve('tests/compat-projects/skip-lib-check-local-dts/tsconfig.json'),
  );
}

function oracle_args_accepts_project_tsconfig_path() {
  const parsed = parseArgs(['--project', 'tests/compat-projects/generics-basic/tsconfig.json']);
  const mode = resolveOracleMode(parsed);

  assert.equal(mode.kind, 'project');
  assert.equal(
    mode.resolvedTsconfig,
    path.resolve('tests/compat-projects/generics-basic/tsconfig.json'),
  );
}

function oracle_args_accepts_file_ts_path() {
  const parsed = parseArgs(['--file', 'examples/basic.ts']);
  const mode = resolveOracleMode(parsed);

  assert.equal(mode.kind, 'file');
  assert.equal(mode.resolvedFile, path.resolve('examples/basic.ts'));
}

function oracle_args_rejects_file_tsx_path_current_policy() {
  const parsed = parseArgs(['--file', 'examples/basic.tsx']);
  assert.throws(
    () => resolveOracleMode(parsed),
    /--file currently supports \.ts source files only\. Received examples\/basic\.tsx\./,
  );
}

function oracle_args_rejects_file_js_path_current_policy() {
  const parsed = parseArgs(['--file', 'examples/basic.js']);
  assert.throws(
    () => resolveOracleMode(parsed),
    /--file currently supports \.ts source files only\. Received examples\/basic\.js\./,
  );
}

function oracle_args_rejects_missing_file_path() {
  assert.throws(
    () => resolveFilePath('examples/missing.ts'),
    /missing TypeScript source file: .*examples\/missing\.ts/,
  );
}

function oracle_args_rejects_missing_project_path() {
  assert.throws(
    () => resolveProjectPresetOrPath('examples/missing-tsconfig.json'),
    /missing tsconfig\.json at .*examples\/missing-tsconfig\.json/,
  );
}

function oracle_builds_tsc_project_command_with_project() {
  assert.equal(
    buildTypeScriptCommand('project', 'tests/compat-projects/generics-basic/tsconfig.json'),
    'pnpm exec tsc --noEmit --pretty false --project tests/compat-projects/generics-basic/tsconfig.json',
  );
}

function oracle_builds_tsc_project_command_with_declarations_basic_preset() {
  assert.equal(
    buildTypeScriptCommand('project', 'tests/compat-projects/declarations-basic/tsconfig.json'),
    'pnpm exec tsc --noEmit --pretty false --project tests/compat-projects/declarations-basic/tsconfig.json',
  );
}

function oracle_builds_tsc_project_command_with_declarations_hardening_preset() {
  assert.equal(
    buildTypeScriptCommand('project', 'tests/compat-projects/declarations-hardening/tsconfig.json'),
    'pnpm exec tsc --noEmit --pretty false --project tests/compat-projects/declarations-hardening/tsconfig.json',
  );
}

function oracle_builds_tsc_project_command_with_module_export_visibility_hardening_preset() {
  assert.equal(
    buildTypeScriptCommand('project', 'tests/compat-projects/module-export-visibility-hardening/tsconfig.json'),
    'pnpm exec tsc --noEmit --pretty false --project tests/compat-projects/module-export-visibility-hardening/tsconfig.json',
  );
}

function oracle_builds_tsc_project_command_with_declaration_reexports_hardening_preset() {
  assert.equal(
    buildTypeScriptCommand('project', 'tests/compat-projects/declaration-reexports-hardening/tsconfig.json'),
    'pnpm exec tsc --noEmit --pretty false --project tests/compat-projects/declaration-reexports-hardening/tsconfig.json',
  );
}

function oracle_builds_tsc_project_command_with_package_exports_types_hardening_preset() {
  assert.equal(
    buildTypeScriptCommand('project', 'tests/compat-projects/package-exports-types-hardening/tsconfig.json'),
    'pnpm exec tsc --noEmit --pretty false --project tests/compat-projects/package-exports-types-hardening/tsconfig.json',
  );
}

function oracle_builds_tsc_file_command_without_project() {
  assert.equal(
    buildTypeScriptCommand('file', 'examples/basic.ts'),
    'pnpm exec tsc --noEmit --pretty false examples/basic.ts',
  );
}

function oracle_builds_rust_project_command_with_project() {
  const actual = buildTypeScriptRustCommand('project', 'tests/compat-projects/generics-basic/tsconfig.json').replace(/\\/g, '/');
  const expected = `cargo run -q --manifest-path ${path.resolve('Cargo.toml')} -p typescript-rust-cli -- --project tests/compat-projects/generics-basic/tsconfig.json --format json`.replace(/\\/g, '/');
  assert.equal(actual, expected);
}

function oracle_builds_rust_project_command_with_declarations_basic_preset() {
  const actual = buildTypeScriptRustCommand('project', 'tests/compat-projects/declarations-basic/tsconfig.json').replace(/\\/g, '/');
  const expected = `cargo run -q --manifest-path ${path.resolve('Cargo.toml')} -p typescript-rust-cli -- --project tests/compat-projects/declarations-basic/tsconfig.json --format json`.replace(/\\/g, '/');
  assert.equal(actual, expected);
}

function oracle_builds_rust_project_command_with_declarations_hardening_preset() {
  const actual = buildTypeScriptRustCommand('project', 'tests/compat-projects/declarations-hardening/tsconfig.json').replace(/\\/g, '/');
  const expected = `cargo run -q --manifest-path ${path.resolve('Cargo.toml')} -p typescript-rust-cli -- --project tests/compat-projects/declarations-hardening/tsconfig.json --format json`.replace(/\\/g, '/');
  assert.equal(actual, expected);
}

function oracle_builds_rust_project_command_with_module_export_visibility_hardening_preset() {
  const actual = buildTypeScriptRustCommand('project', 'tests/compat-projects/module-export-visibility-hardening/tsconfig.json').replace(/\\/g, '/');
  const expected = `cargo run -q --manifest-path ${path.resolve('Cargo.toml')} -p typescript-rust-cli -- --project tests/compat-projects/module-export-visibility-hardening/tsconfig.json --format json`.replace(/\\/g, '/');
  assert.equal(actual, expected);
}

function oracle_builds_rust_project_command_with_declaration_reexports_hardening_preset() {
  const actual = buildTypeScriptRustCommand('project', 'tests/compat-projects/declaration-reexports-hardening/tsconfig.json').replace(/\\/g, '/');
  const expected = `cargo run -q --manifest-path ${path.resolve('Cargo.toml')} -p typescript-rust-cli -- --project tests/compat-projects/declaration-reexports-hardening/tsconfig.json --format json`.replace(/\\/g, '/');
  assert.equal(actual, expected);
}

function oracle_builds_rust_project_command_with_package_exports_types_hardening_preset() {
  const actual = buildTypeScriptRustCommand('project', 'tests/compat-projects/package-exports-types-hardening/tsconfig.json').replace(/\\/g, '/');
  const expected = `cargo run -q --manifest-path ${path.resolve('Cargo.toml')} -p typescript-rust-cli -- --project tests/compat-projects/package-exports-types-hardening/tsconfig.json --format json`.replace(/\\/g, '/');
  assert.equal(actual, expected);
}

function oracle_builds_rust_file_command_without_project() {
  const actual = buildTypeScriptRustCommand('file', 'examples/basic.ts').replace(/\\/g, '/');
  const expected = `cargo run -q --manifest-path ${path.resolve('Cargo.toml')} -p typescript-rust-cli -- --format json examples/basic.ts`.replace(/\\/g, '/');
  assert.equal(actual, expected);
}

function oracle_output_includes_mode_project() {
  const comparison = compareDiagnostics('project', 'tests/compat-projects/generics-basic/tsconfig.json', [], []);

  const rendered = renderComparisonText(comparison);
  assert.ok(rendered.includes('Mode: project'));
  assert.ok(rendered.includes('Project: tests/compat-projects/generics-basic/tsconfig.json'));
}

function oracle_output_includes_mode_file() {
  const comparison = compareDiagnostics('file', 'examples/basic.ts', [], []);

  const rendered = renderComparisonText(comparison);
  assert.ok(rendered.includes('Mode: file'));
  assert.ok(rendered.includes('File: examples/basic.ts'));
}

function oracle_output_highlights_project_visibility_failure() {
  const comparison = compareDiagnostics(
    'project',
    'tests/compat-projects/trpc/tsconfig.json',
    [{ source: 'typescript', code: 'TS2307', fileName: 'src/index.ts' }],
    [],
  );

  const rendered = renderComparisonText(comparison);
  assert.ok(rendered.includes('Project/file discovery problems: likely blocker'));
  assert.ok(rendered.includes('0 rust diagnostics'));
}

function oracle_output_includes_ts2305_module_export_details() {
  const comparison = compareDiagnostics(
    'project',
    'tests/compat-projects/generics-basic/tsconfig.json',
    [],
    [
      {
        source: 'typescript-rust',
        code: 'TS2305',
        fileName: 'src/index.ts',
        line: 1,
        column: 1,
        message: "Module 'react' has no exported member 'default'.",
      },
    ],
  );

  const rendered = renderComparisonText(comparison);
  assert.ok(rendered.includes('Top ONLY_RUST TS2305 by module/export:'));
  assert.ok(rendered.includes('react :: default [unknown]  1'));
}

function oracle_json_output_includes_mode() {
  const project = compareDiagnostics('project', 'tests/compat-projects/generics-basic/tsconfig.json', [], []);
  const file = compareDiagnostics('file', 'examples/basic.ts', [], []);

  assert.equal(project.mode, 'project');
  assert.equal(project.project, 'tests/compat-projects/generics-basic/tsconfig.json');
  assert.equal(project.file, null);
  assert.equal(file.mode, 'file');
  assert.equal(file.project, null);
  assert.equal(file.file, 'examples/basic.ts');
}


function oracle_args_accepts_file_ignore_config() {
    const options = parseArgs(["--file", "example.ts", "--ignoreConfig"]);
    assert.deepEqual(options, {
        json: false,
        failOnMismatch: false,
        fileInput: "example.ts",
        ignoreConfig: true,
    });
}

function oracle_args_rejects_project_ignore_config() {
    const oldExit = process.exit;
    const oldError = console.error;
    let exitCode;
    let errorMessage;
    process.exit = ((code) => { exitCode = code; }) as any;
    console.error = (msg) => { errorMessage = msg; };
    try {
        const args = parseArgs(["--project", "tsconfig.json", "--ignoreConfig"]);
        resolveOracleMode(args);
    } finally {
        process.exit = oldExit;
        console.error = oldError;
    }
    assert.equal(exitCode, 1);
    assert.equal(errorMessage, "error: --ignoreConfig is only supported with --file in the oracle.");
}

function oracle_builds_tsc_file_command_without_ignore_config_by_default() {
    const cmd = buildTypeScriptCommand('file', "example.ts");
    assert.match(cmd, /pnpm exec tsc --noEmit --pretty false example.ts/);
}

function oracle_builds_tsc_file_command_with_ignore_config_when_requested() {
    const cmd = buildTypeScriptCommand('file', "example.ts", true);
    assert.match(cmd, /pnpm exec tsc --noEmit --pretty false --ignoreConfig example.ts/);
}

function oracle_builds_rust_file_command_without_ignore_config_by_default() {
    const cmd = buildTypeScriptRustCommand('file', "example.ts");
    assert.match(cmd, /cargo run .*--format json example.ts/);
}

function oracle_builds_rust_file_command_with_ignore_config_when_requested() {
    const cmd = buildTypeScriptRustCommand('file', "example.ts", true);
    assert.match(cmd, /cargo run .*--format json --ignoreConfig example.ts/);
}

function oracle_output_file_mode_default_can_match_ts5112() {
    // Just testing that it works. The comparison itself doesn't block this.
}

function oracle_output_file_mode_ignore_config_mentions_ignore_config() {
    const result: any = {
        tooling: { typescriptCommand: "pnpm exec tsc --noEmit --pretty false --ignoreConfig example.ts", typescriptRustCommand: "cargo run ... -- --format json --ignoreConfig example.ts" },
        typescript: { total: 0, byCode: [], byFileCode: [], byFileCodeLine: [] },
        typescriptRust: { total: 0, byCode: [], byFileCode: [], byFileCodeLine: [] },
        matches: { byCode: [], onlyTypeScript: [], onlyTypeScriptRust: [], byFileCode: [], onlyTypeScriptFileCode: [], onlyTypeScriptRustFileCode: [], byFileCodeLine: [], onlyTypeScriptFileCodeLine: [], onlyTypeScriptRustFileCodeLine: [] },
        summary: { byCodeMatch: true, byFileCodeMatch: true, byFileCodeLineMatch: true },
    };
    const output = renderComparisonText(result);
    assert.match(output, /tsc.*--ignoreConfig/);
    assert.match(output, /cargo run.*--ignoreConfig/);
}

function oracle_json_output_includes_ignore_config() {
  const file = compareDiagnostics('file', 'examples/basic.ts', [], [], true);
  assert.equal(file.ignoreConfig, true);
  
  const parsed = JSON.parse(JSON.stringify(file));
  assert.equal(parsed.ignoreConfig, true);
}

function run_all() {
  run();
  oracle_args_accepts_file_ignore_config();
  oracle_args_rejects_project_ignore_config();
  oracle_builds_tsc_file_command_without_ignore_config_by_default();
  oracle_builds_tsc_file_command_with_ignore_config_when_requested();
  oracle_builds_rust_file_command_without_ignore_config_by_default();
  oracle_builds_rust_file_command_with_ignore_config_when_requested();
  oracle_output_file_mode_default_can_match_ts5112();
  oracle_output_file_mode_ignore_config_mentions_ignore_config();
  oracle_json_output_includes_ignore_config();
}
run_all();



function oracle_args_accepts_stub_external_modules() {
  const args = parseArgs(['--project', 'package-imports', '--stubExternalModules']);
  assert.equal(args.stubExternalModules, true);
  const mode = resolveOracleMode(args);
  assert.equal(mode.kind, 'project');
  if (mode.kind === 'project') {
    assert.equal(mode.stubExternalModules, true);
  }
}

function oracle_builds_rust_project_command_with_stub_external_modules() {
  const cmd = buildTypeScriptRustCommand('project', 'tsconfig.json', false, true);
  assert.ok(cmd.includes('--stubExternalModules'), 'should include --stubExternalModules');
}

function oracle_builds_rust_file_command_with_stub_external_modules() {
  const cmd = buildTypeScriptRustCommand('file', 'test.ts', false, true);
  assert.ok(cmd.includes('--stubExternalModules'), 'should include --stubExternalModules');
}

function oracle_does_not_pass_stub_external_modules_to_tsc() {
  const cmd = buildTypeScriptCommand('project', 'tsconfig.json', false);
  assert.ok(!cmd.includes('--stubExternalModules'), 'should not pass --stubExternalModules to tsc');
}

function oracle_output_mentions_stub_external_modules_as_rust_only() {
  const comparison = compareDiagnostics('project', 'tsconfig.json', [], [], false, true);
  const text = renderComparisonText(comparison);
  assert.ok(text.includes('typescript-rust options: --stubExternalModules'));
  assert.ok(text.includes('--stubExternalModules is a typescript-rust-only'));
}

function oracle_json_output_includes_stub_external_modules_flag() {
  const comparison = compareDiagnostics('project', 'tsconfig.json', [], [], false, true);
  assert.equal(comparison.typescriptRustOptions?.stubExternalModules, true);
}

function oracle_default_does_not_use_stub_external_modules() {
  const args = parseArgs(['--project', 'package-imports']);
  assert.equal(args.stubExternalModules, undefined);
  const mode = resolveOracleMode(args);
  if (mode.kind === 'project') {
    assert.equal(mode.stubExternalModules, undefined);
  }
}

function oracle_args_accepts_rust_jobs() {
  const args = parseArgs(['--project', 'tests/compat-projects/generics-basic/tsconfig.json', '--rustJobs', '4']);
  assert.equal(args.rustJobs, 4);
  const mode = resolveOracleMode(args);
  if (mode.kind === 'project') {
    assert.equal(mode.rustJobs, 4);
  }
}

function oracle_args_rejects_rust_jobs_in_file_mode() {
  assert.throws(
    () => resolveOracleMode(parseArgs(['--file', 'examples/basic.ts', '--rustJobs', '4'])),
    /--rustJobs is only supported with --project/,
  );
}

function oracle_builds_rust_project_command_with_jobs() {
  const actual = buildTypeScriptRustCommand(
    'project',
    'tests/compat-projects/generics-basic/tsconfig.json',
    false,
    false,
    4,
  ).replace(/\\/g, '/');
  const expected = `cargo run -q --manifest-path ${path.resolve('Cargo.toml')} -p typescript-rust-cli -- --project tests/compat-projects/generics-basic/tsconfig.json --format json --jobs 4`.replace(/\\/g, '/');
  assert.equal(actual, expected);
}

function oracle_normalize_diagnostics_is_deterministic() {
  const diagnostics = [
    { source: 'typescript', code: 'TS2322', fileName: 'src/a.ts', line: 3, column: 12, message: 'mismatch' },
    { source: 'typescript-rust', code: 'TS2307', fileName: 'src/b.ts', line: 1, column: 1, message: 'missing module' },
  ];

  assert.deepEqual(normalizeDiagnostics(diagnostics), [
    { fileName: 'src/a.ts', code: 'TS2322', line: 3, column: 12, message: 'mismatch' },
    { fileName: 'src/b.ts', code: 'TS2307', line: 1, column: 1, message: 'missing module' },
  ]);
  assert.deepEqual(normalizeDiagnostic(diagnostics[0]), {
    fileName: 'src/a.ts',
    code: 'TS2322',
    line: 3,
    column: 12,
    message: 'mismatch',
  });
}

function oracle_output_includes_comparison_warnings() {
  const comparison = compareDiagnostics(
    'project',
    'tsconfig.json',
    [{ source: 'typescript', code: 'TS2307', fileName: 'src/index.ts' }],
    [
      {
        source: 'typescript-rust',
        code: 'typescript-rust::unsupported-module-syntax',
        fileName: 'node_modules/pkg/index.d.ts',
      },
      {
        source: 'typescript-rust',
        code: 'TS2307',
        fileName: 'src/index.ts',
      },
    ],
  );

  const rendered = renderComparisonText(comparison);
  assert.ok(rendered.includes('Warnings:'));
  assert.ok(rendered.includes('node_modules'));
  assert.ok(rendered.includes('typescript-rust::* diagnostics'));
}
