import assert from 'node:assert';
import { mkdtempSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

import {
  caseConfig,
  caseFingerprint,
  parseUpstreamCase,
  selectHeldOutCases,
  type OwnTestSuite,
} from './upstream-cases.js';

const typescriptPath = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../node_modules/typescript-6/lib/typescript.js',
);

test('splits units on @filename and keeps directives out of the source', () => {
  const parsed = parseUpstreamCase(
    ['// @strict: true', '// @filename: a.ts', 'export const a = 1;', '// @Filename: /b.ts', 'import { a } from "./a";'].join('\n'),
    'case.ts',
  );
  assert.deepStrictEqual(parsed.directives, [['strict', 'true']]);
  assert.deepStrictEqual(parsed.files, [
    { name: 'a.ts', content: 'export const a = 1;' },
    { name: '/b.ts', content: 'import { a } from "./a";' },
  ]);
});

test('a case without @filename is one unit named after the case', () => {
  const parsed = parseUpstreamCase('// @target: es2015\nlet x = 1;', 'single.ts');
  assert.deepStrictEqual(parsed.files, [{ name: 'single.ts', content: 'let x = 1;' }]);
});

test('options take their tsc spelling, the first variation, and drop emit-only settings', () => {
  const config = caseConfig(
    parseUpstreamCase(
      '// @STRICT: false\n// @target: es2015, esnext\n// @lib: es2015, dom\n// @outFile: out.js\n// @module: *\n// @noTypesAndSymbols: true\nlet x = 1;',
      'c.ts',
    ),
    typescriptPath,
  );
  assert.ok(!('skip' in config));
  assert.deepStrictEqual(config.compilerOptions, { strict: false, target: 'es2015', lib: ['es2015', 'dom'], noEmit: true });
  assert.deepStrictEqual(config.files, ['c.ts']);
});

test('node_modules units and scripts without allowJs are not roots', () => {
  const config = caseConfig(
    parseUpstreamCase('// @filename: /node_modules/p/index.d.ts\nexport {};\n// @filename: main.js\nx;\n// @filename: main.ts\nexport {};', 'c.ts'),
    typescriptPath,
  );
  assert.ok(!('skip' in config));
  assert.deepStrictEqual(config.files, ['main.ts']);
});

test('harness-only directives and a case-supplied tsconfig are skipped', () => {
  assert.ok('skip' in caseConfig(parseUpstreamCase('// @link: /a -> /b\nx;', 'c.ts'), typescriptPath));
  assert.ok('skip' in caseConfig(parseUpstreamCase('// @filename: tsconfig.json\n{}\n// @filename: a.ts\nx;', 'c.ts'), typescriptPath));
});

test('a case in either checker suite is held out, by name or by content', () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'upstream-cases-'));
  const write = (name: string, text: string) => {
    const file = path.join(dir, `${name}.ts`);
    writeFileSync(file, text);
    return { name, path: file };
  };
  const pool = [write('byName', 'let a = 1;'), write('byContent', '// @strict: true\nlet   b = 2;'), write('free', 'let c = 3;')];
  const suites: OwnTestSuite[] = [
    { label: 'bolt-ts', names: new Set(['byName']), fingerprints: new Set() },
    { label: 'surge-ts', names: new Set(), fingerprints: new Set([caseFingerprint('let b = 2;')]) },
  ];
  const selection = selectHeldOutCases(pool, suites, 0);
  assert.deepStrictEqual(selection.cases.map((source) => source.name), ['free']);
  assert.deepStrictEqual(selection.excluded, { 'bolt-ts': 1, 'surge-ts': 1 });
});
