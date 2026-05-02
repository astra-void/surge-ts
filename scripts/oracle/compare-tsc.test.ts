import assert from 'node:assert/strict';

import {
  compareDiagnostics,
  countDiagnostics,
  parseArgs,
  parseTypeScriptDiagnostics,
  parseTypeScriptRustDiagnostics,
  resolveProjectInput,
} from './compare-tsc.ts';

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
  oracle_compare_match();
  oracle_compare_only_typescript();
  oracle_compare_only_typescript_rust();
  oracle_unknown_project_fails_cleanly();
  oracle_parse_args_strict_codes_alias();
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

function oracle_compare_match() {
  const comparison = compareDiagnostics(
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
    [],
    [{ source: 'typescript-rust', code: 'TS2322', fileName: 'src/a.ts' }],
  );

  assert.equal(comparison.summary.byCodeMatch, false);
  assert.equal(comparison.matches.onlyTypeScript.length, 0);
  assert.equal(comparison.matches.onlyTypeScriptRust.length, 1);
}

function oracle_unknown_project_fails_cleanly() {
  assert.throws(
    () => resolveProjectInput('does-not-exist'),
    /unknown project preset "does-not-exist"/,
  );
}

function oracle_parse_args_strict_codes_alias() {
  const parsed = parseArgs(['--project', 'generics-basic', '--strictCodes']);
  assert.equal(parsed.failOnMismatch, true);
}

run();
