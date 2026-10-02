import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { expect, test } from 'vitest';

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

test('parses typescript output', () => {
  const diagnostics = parseTypeScriptDiagnostics(
    'src/index.ts(3,12): error TS2322: Type "number" is not assignable to type "string".',
    '/repo',
  );

  expect(diagnostics.length).toBe(1);
  expect(diagnostics[0]).toStrictEqual({
    source: 'typescript',
    code: 'TS2322',
    fileName: 'src/index.ts',
    line: 3,
    column: 12,
    message: 'Type "number" is not assignable to type "string".',
  });
});

test('parses rust output', () => {
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

  expect(diagnostics.length).toBe(1);
  expect(diagnostics[0]).toStrictEqual({
    source: 'surge-ts',
    code: 'TS2307',
    fileName: 'src/index.ts',
    line: 1,
    column: 1,
    message: "Cannot find module 'pkg' or its corresponding type declarations.",
  });
});

test('counts diagnostic keys', () => {
  const counts = countDiagnostics(
    [
      { source: 'typescript', code: 'TS2322', fileName: 'src/a.ts' },
      { source: 'typescript', code: 'TS2322', fileName: 'src/b.ts' },
      { source: 'surge-ts', code: 'TS2304', fileName: 'src/a.ts' },
    ],
    (diagnostic) => diagnostic.code,
  );

  expect(counts.get('TS2322')).toBe(2);
  expect(counts.get('TS2304')).toBe(1);
});

test('extracts raw message fields', () => {
  expect(extractTs2305ModuleExport("Module 'pkg' has no exported member 'Thing'.")).toStrictEqual(
    { moduleSpecifier: 'pkg', exportName: 'Thing' },
  );
  expect(
    extractTs2307ModuleSpecifier("Cannot find module 'pkg' or its corresponding type declarations."),
  ).toBe('pkg');
  expect(extractTs2304Identifier("Cannot find name 'missingValue'.")).toBe('missingValue');
});

test('resolves project and file inputs', () => {
  const project = createProject();

  const projectPath = resolveProjectPresetOrPath(project.root);
  expect(projectPath).toBe(project.tsconfig);

  const projectMode = resolveOracleMode(parseArgs(['--project', project.root]));
  expect(projectMode).toMatchObject({ kind: 'project', resolvedTsconfig: project.tsconfig });

  const fileMode = resolveOracleMode(parseArgs(['--file', project.entry]));
  expect(fileMode).toMatchObject({ kind: 'file', resolvedFile: project.entry });

  expect(resolveFilePath(project.entry)).toBe(project.entry);
});

test('builds commands', () => {
  expect(
    buildTypeScriptCommand('project', 'tests/compat-projects/generics-basic/tsconfig.json'),
  ).toBe(
    'node node_modules/typescript/bin/tsc --noEmit --pretty false --project tests/compat-projects/generics-basic/tsconfig.json',
  );
  expect(buildTypeScriptCommand('file', 'examples/basic.ts', true)).toBe(
    'node node_modules/typescript/bin/tsc --noEmit --pretty false --ignoreConfig examples/basic.ts',
  );
  expect(
    buildSurgeTsCommand('project', 'tests/compat-projects/generics-basic/tsconfig.json').replace(/\\/g, '/'),
  ).toMatch(
    /cargo run -q --manifest-path .*Cargo\.toml -p surge-ts-cli -- --project tests\/compat-projects\/generics-basic\/tsconfig\.json --format json/,
  );
  expect(buildSurgeTsCommand('file', 'examples/basic.ts', true).replace(/\\/g, '/')).toMatch(
    /cargo run -q --manifest-path .*Cargo\.toml -p surge-ts-cli -- --format json --ignoreConfig examples\/basic\.ts/,
  );
});

test('compares raw fingerprints', () => {
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

  expect(comparison.summary.byCodeMatch).toBe(false);
  expect(comparison.details?.onlyTypeScript?.rawDiagnosticFingerprints?.length).toBe(1);
  expect(comparison.details?.onlySurgeTs?.rawDiagnosticFingerprints?.length).toBe(4);
  expect(comparison.details?.onlySurgeTs?.rawDiagnosticFingerprints?.[0]?.code).toBe('TS2322');
  expect(comparison.details?.onlySurgeTs?.rawTs2307ModuleSpecifiers?.[0].key).toBe('pkg');
  expect(comparison.details?.onlySurgeTs?.rawTs2304Identifiers?.[0].key).toBe('missingValue');
  expect(comparison.details?.onlySurgeTs?.rawTs2305ModuleExports?.[0].moduleSpecifier).toBe('pkg');
  expect(
    formatDiagnosticFingerprintEntry(comparison.details?.onlySurgeTs?.rawDiagnosticFingerprints?.[0] ?? {
      fileName: 'src/right.ts',
      code: 'TS2322',
      line: 1,
      column: 1,
      message: 'Type mismatch',
      count: 1,
    }),
  ).toBe('src/right.ts:1:1 TS2322 1 Type mismatch');
});

test('compares message parity', () => {
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
  expect(parity.comparedLocations).toBe(2);
  expect(parity.matches).toBe(1);
  expect(parity.mismatches.length).toBe(1);
  expect(parity.mismatches[0]).toStrictEqual({
    fileName: 'src/a.ts',
    code: 'TS2345',
    line: 7,
    column: 7,
    typescript: "Argument of type 'number' is not assignable to parameter of type 'string'.",
    surgeTs: "Argument of type '1' is not assignable to parameter of type 'string'.",
  });
});

test('renders message parity', () => {
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

  expect(comparison.summary.messageMatch).toBe(false);
  expect(comparison.messageParity.mismatches.length).toBe(1);

  const rendered = renderComparisonText(comparison);
  expect(rendered).toContain('Message match: no');
  expect(rendered).toContain('Message parity (same file/code/line/column, message text differs):');
  expect(rendered).toContain("tsc : Argument of type 'number'");
  expect(rendered).toContain("rust: Argument of type '1'");
});

test('renders raw sections', () => {
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
  expect(rendered).toContain('Summary:');
  expect(rendered).toContain('Raw message extraction, not root-cause classification:');
  expect(rendered).toContain('Top ONLY_RUST raw diagnostic fingerprints:');
  expect(rendered).toContain('Top ONLY_TS raw diagnostic fingerprints:');
  expect(rendered).toContain('TS2307 specifiers:');
});
