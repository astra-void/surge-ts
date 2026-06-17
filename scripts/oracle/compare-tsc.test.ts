import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import {
  buildTypeScriptCommand,
  buildSurgeTsCommand,
  compareDiagnostics,
  compareMessages,
  countDiagnostics,
  extractTs2304Identifier,
  extractTs2305ModuleExport,
  extractTs2307ModuleSpecifier,
  formatDiagnosticFingerprintEntry,
  parseArgs,
  parseTypeScriptDiagnostics,
  parseSurgeTsDiagnostics,
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
  compares_message_parity();
  renders_message_parity();
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
  const diagnostics = parseSurgeTsDiagnostics(
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
    source: 'surge-ts',
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
      { source: 'surge-ts', code: 'TS2304', fileName: 'src/a.ts' },
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
    buildSurgeTsCommand('project', 'tests/compat-projects/generics-basic/tsconfig.json').replace(/\\/g, '/'),
    /cargo run -q --manifest-path .*Cargo\.toml -p surge-ts-cli -- --project tests\/compat-projects\/generics-basic\/tsconfig\.json --format json/,
  );
  assert.match(
    buildSurgeTsCommand('file', 'examples/basic.ts', true).replace(/\\/g, '/'),
    /cargo run -q --manifest-path .*Cargo\.toml -p surge-ts-cli -- --format json --ignoreConfig examples\/basic\.ts/,
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
        source: 'surge-ts',
        code: 'TS2322',
        fileName: 'src/right.ts',
        line: 1,
        column: 1,
        message: 'Type mismatch',
      },
      {
        source: 'surge-ts',
        code: 'TS2307',
        fileName: 'src/right.ts',
        line: 2,
        column: 8,
        message: "Cannot find module 'pkg' or its corresponding type declarations.",
      },
      {
        source: 'surge-ts',
        code: 'TS2304',
        fileName: 'src/right.ts',
        line: 3,
        column: 12,
        message: "Cannot find name 'missingValue'.",
      },
      {
        source: 'surge-ts',
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
  assert.equal(comparison.details?.onlySurgeTs?.rawDiagnosticFingerprints?.length, 4);
  assert.equal(comparison.details?.onlySurgeTs?.rawDiagnosticFingerprints?.[0]?.code, 'TS2322');
  assert.equal(comparison.details?.onlySurgeTs?.rawTs2307ModuleSpecifiers?.[0].key, 'pkg');
  assert.equal(comparison.details?.onlySurgeTs?.rawTs2304Identifiers?.[0].key, 'missingValue');
  assert.equal(comparison.details?.onlySurgeTs?.rawTs2305ModuleExports?.[0].moduleSpecifier, 'pkg');
  assert.equal(
    formatDiagnosticFingerprintEntry(comparison.details?.onlySurgeTs?.rawDiagnosticFingerprints?.[0] ?? {
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

function compares_message_parity(): void {
  const parity = compareMessages(
    [
      // Same location, message text differs (widened vs literal type).
      { source: 'typescript', code: 'TS2345', fileName: 'src/a.ts', line: 7, column: 7, message: "Argument of type 'number' is not assignable to parameter of type 'string'." },
      // Same location, identical message.
      { source: 'typescript', code: 'TS2554', fileName: 'src/a.ts', line: 6, column: 1, message: 'Expected 1 arguments, but got 2.' },
      // Location only on the TypeScript side (different column) -> not message-compared.
      { source: 'typescript', code: 'TS2322', fileName: 'src/a.ts', line: 9, column: 5, message: "Type 'number' is not assignable to type 'string'." },
    ],
    [
      { source: 'surge-ts', code: 'TS2345', fileName: 'src/a.ts', line: 7, column: 7, message: "Argument of type '1' is not assignable to parameter of type 'string'." },
      { source: 'surge-ts', code: 'TS2554', fileName: 'src/a.ts', line: 6, column: 1, message: 'Expected 1 arguments, but got 2.' },
      { source: 'surge-ts', code: 'TS2322', fileName: 'src/a.ts', line: 9, column: 26, message: "Type '1' is not assignable to type 'string'." },
    ],
  );

  // Two locations share an exact (file, code, line, column): TS2345 and TS2554.
  assert.equal(parity.comparedLocations, 2);
  assert.equal(parity.matches, 1);
  assert.equal(parity.mismatches.length, 1);
  assert.deepEqual(parity.mismatches[0], {
    fileName: 'src/a.ts',
    code: 'TS2345',
    line: 7,
    column: 7,
    typescript: "Argument of type 'number' is not assignable to parameter of type 'string'.",
    surgeTs: "Argument of type '1' is not assignable to parameter of type 'string'.",
  });
}

function renders_message_parity(): void {
  const comparison = compareDiagnostics(
    'project',
    'tests/compat-projects/sample/tsconfig.json',
    [
      { source: 'typescript', code: 'TS2345', fileName: 'src/a.ts', line: 7, column: 7, message: "Argument of type 'number' is not assignable to parameter of type 'string'." },
    ],
    [
      { source: 'surge-ts', code: 'TS2345', fileName: 'src/a.ts', line: 7, column: 7, message: "Argument of type '1' is not assignable to parameter of type 'string'." },
    ],
  );

  assert.equal(comparison.summary.messageMatch, false);
  assert.equal(comparison.messageParity.mismatches.length, 1);

  const rendered = renderComparisonText(comparison);
  assert.ok(rendered.includes('Message match: no'));
  assert.ok(rendered.includes('Message parity (same file/code/line/column, message text differs):'));
  assert.ok(rendered.includes("tsc : Argument of type 'number'"));
  assert.ok(rendered.includes("rust: Argument of type '1'"));
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
        source: 'surge-ts',
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
