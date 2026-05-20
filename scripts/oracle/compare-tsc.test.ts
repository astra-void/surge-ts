import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import {
  buildTypeScriptCommand,
  buildTypeScriptRustCommand,
  compareDiagnostics,
  countDiagnostics,
  extractTs2304Identifier,
  extractTs2305ModuleExport,
  extractTs2307ModuleSpecifier,
  formatDiagnosticFingerprintEntry,
  parseArgs,
  parseTypeScriptDiagnostics,
  parseTypeScriptRustDiagnostics,
  renderComparisonText,
  resolveFilePath,
  resolveOracleMode,
  resolveProjectPresetOrPath,
} from './compare-tsc';

function run(): void {
  parses_typescript_output();
  parses_rust_output();
  counts_diagnostic_keys();
  extracts_raw_message_fields();
  resolves_project_and_file_inputs();
  builds_commands();
  compares_raw_fingerprints();
  renders_raw_sections();
}

function tempDir(prefix: string): string {
  return fs.mkdtempSync(path.join(os.tmpdir(), prefix));
}

function createProject(): { root: string; tsconfig: string; entry: string } {
  const root = tempDir('oracle-compare-');
  const tsconfig = path.join(root, 'tsconfig.json');
  const entry = path.join(root, 'src', 'index.ts');
  fs.mkdirSync(path.dirname(entry), { recursive: true });
  fs.writeFileSync(
    tsconfig,
    JSON.stringify({ compilerOptions: { strict: true, noEmit: true }, include: ['src/**/*.ts'] }),
  );
  fs.writeFileSync(entry, 'export const value: number = "x";\n');
  return { root, tsconfig, entry };
}

function parses_typescript_output(): void {
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

function parses_rust_output(): void {
  const diagnostics = parseTypeScriptRustDiagnostics(
    JSON.stringify({
      diagnostics: [
        {
          code: 'TS2307',
          fileName: '/repo/src/index.ts',
          line: 1,
          column: 1,
          message: "Cannot find module 'pkg' or its corresponding type declarations.",
        },
      ],
    }),
    '/repo',
  );

  assert.equal(diagnostics.length, 1);
  assert.deepEqual(diagnostics[0], {
    source: 'typescript-rust',
    code: 'TS2307',
    fileName: 'src/index.ts',
    line: 1,
    column: 1,
    message: "Cannot find module 'pkg' or its corresponding type declarations.",
  });
}

function counts_diagnostic_keys(): void {
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

function extracts_raw_message_fields(): void {
  assert.deepEqual(
    extractTs2305ModuleExport("Module 'pkg' has no exported member 'Thing'."),
    { moduleSpecifier: 'pkg', exportName: 'Thing' },
  );
  assert.equal(
    extractTs2307ModuleSpecifier("Cannot find module 'pkg' or its corresponding type declarations."),
    'pkg',
  );
  assert.equal(extractTs2304Identifier("Cannot find name 'missingValue'."), 'missingValue');
}

function resolves_project_and_file_inputs(): void {
  const project = createProject();

  const projectPath = resolveProjectPresetOrPath(project.root);
  assert.equal(projectPath, project.tsconfig);

  const projectMode = resolveOracleMode(parseArgs(['--project', project.root]));
  assert.equal(projectMode.kind, 'project');
  assert.equal(projectMode.resolvedTsconfig, project.tsconfig);

  const fileMode = resolveOracleMode(parseArgs(['--file', project.entry]));
  assert.equal(fileMode.kind, 'file');
  assert.equal(fileMode.resolvedFile, project.entry);

  assert.equal(resolveFilePath(project.entry), project.entry);
}

function builds_commands(): void {
  assert.equal(
    buildTypeScriptCommand('project', 'tests/compat-projects/generics-basic/tsconfig.json'),
    'pnpm exec tsc --noEmit --pretty false --project tests/compat-projects/generics-basic/tsconfig.json',
  );
  assert.equal(
    buildTypeScriptCommand('file', 'examples/basic.ts', true),
    'pnpm exec tsc --noEmit --pretty false --ignoreConfig examples/basic.ts',
  );
  assert.match(
    buildTypeScriptRustCommand('project', 'tests/compat-projects/generics-basic/tsconfig.json').replace(/\\/g, '/'),
    /cargo run -q --manifest-path .*Cargo\.toml -p typescript-rust-cli -- --project tests\/compat-projects\/generics-basic\/tsconfig\.json --format json/,
  );
  assert.match(
    buildTypeScriptRustCommand('file', 'examples/basic.ts', true).replace(/\\/g, '/'),
    /cargo run -q --manifest-path .*Cargo\.toml -p typescript-rust-cli -- --format json --ignoreConfig examples\/basic\.ts/,
  );
}

function compares_raw_fingerprints(): void {
  const comparison = compareDiagnostics(
    'project',
    'tests/compat-projects/sample/tsconfig.json',
    [
      {
        source: 'typescript',
        code: 'TS2322',
        fileName: 'src/left.ts',
        line: 1,
        column: 1,
        message: 'Type mismatch',
      },
    ],
    [
      {
        source: 'typescript-rust',
        code: 'TS2322',
        fileName: 'src/right.ts',
        line: 1,
        column: 1,
        message: 'Type mismatch',
      },
      {
        source: 'typescript-rust',
        code: 'TS2307',
        fileName: 'src/right.ts',
        line: 2,
        column: 8,
        message: "Cannot find module 'pkg' or its corresponding type declarations.",
      },
      {
        source: 'typescript-rust',
        code: 'TS2304',
        fileName: 'src/right.ts',
        line: 3,
        column: 12,
        message: "Cannot find name 'missingValue'.",
      },
      {
        source: 'typescript-rust',
        code: 'TS2305',
        fileName: 'src/right.ts',
        line: 4,
        column: 3,
        message: "Module 'pkg' has no exported member 'Thing'.",
      },
    ],
  );

  assert.equal(comparison.summary.byCodeMatch, false);
  assert.equal(comparison.details?.onlyTypeScript?.rawDiagnosticFingerprints?.length, 1);
  assert.equal(comparison.details?.onlyTypeScriptRust?.rawDiagnosticFingerprints?.length, 4);
  assert.equal(comparison.details?.onlyTypeScriptRust?.rawDiagnosticFingerprints?.[0]?.code, 'TS2322');
  assert.equal(comparison.details?.onlyTypeScriptRust?.rawTs2307ModuleSpecifiers?.[0].key, 'pkg');
  assert.equal(comparison.details?.onlyTypeScriptRust?.rawTs2304Identifiers?.[0].key, 'missingValue');
  assert.equal(comparison.details?.onlyTypeScriptRust?.rawTs2305ModuleExports?.[0].moduleSpecifier, 'pkg');
  assert.equal(
    formatDiagnosticFingerprintEntry(comparison.details?.onlyTypeScriptRust?.rawDiagnosticFingerprints?.[0] ?? {
      fileName: 'src/right.ts',
      code: 'TS2322',
      line: 1,
      column: 1,
      message: 'Type mismatch',
      count: 1,
    }),
    'src/right.ts:1:1 TS2322 1 Type mismatch',
  );
}

function renders_raw_sections(): void {
  const comparison = compareDiagnostics(
    'project',
    'tests/compat-projects/sample/tsconfig.json',
    [
      {
        source: 'typescript',
        code: 'TS2322',
        fileName: 'src/left.ts',
        line: 1,
        column: 1,
        message: 'Type mismatch',
      },
    ],
    [
      {
        source: 'typescript-rust',
        code: 'TS2307',
        fileName: 'src/right.ts',
        line: 2,
        column: 8,
        message: "Cannot find module 'pkg' or its corresponding type declarations.",
      },
    ],
  );

  const rendered = renderComparisonText(comparison);
  assert.ok(rendered.includes('Summary:'));
  assert.ok(rendered.includes('Raw message extraction, not root-cause classification:'));
  assert.ok(rendered.includes('Top ONLY_RUST raw diagnostic fingerprints:'));
  assert.ok(rendered.includes('Top ONLY_TS raw diagnostic fingerprints:'));
  assert.ok(rendered.includes('TS2307 specifiers:'));
}

run();
