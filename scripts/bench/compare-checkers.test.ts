import assert from 'node:assert';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

import type { NormalizedDiagnostic } from '../oracle/compare-tsc.js';
import {
  aggregate,
  codeForMessage,
  compileTemplate,
  loadMessageTemplates,
  parseBoltOutput,
  renderCheckerSvg,
  runProcess,
  scoreDiagnostics,
  translateConfigForBolt,
  type TargetResult,
} from './compare-checkers.js';

const boltOutput = [
  "  × Type 'number[]' is not assignable to type 'Iterable<string, any, any>'.",
  '    ╭─[src/index.ts:15:14]',
  ' 15 │ wrongElement',
  '    · ────────────',
  '    ╰────',
  '',
  "  × Non-abstract class 'MissingGeneric' does not implement inherited abstract",
  "  │ member 'handle' from class 'Generic<string>'.",
  '    ╭─[src/index.ts:54:8]',
  ' 54 │ class MissingGeneric extends Generic<string> {}',
  '    ╰────',
  '',
  'Advice: ',
  "  ☞ Class 'Later' is defined here",
  '    ╭─[src/index.ts:18:7]',
  '    ╰────',
  '',
  "  × Cannot find name 'Iterator'.",
  "  [Failed to read contents for label (offset: 71948, length: 7): OutOfBounds]",
  '',
  'Files: 58',
  'Types: 287',
  'Time cost: 17ms',
].join('\n');

test('parses miette reports, joining wrapped messages and skipping advice', () => {
  assert.deepStrictEqual(parseBoltOutput(boltOutput), [
    {
      message: "Type 'number[]' is not assignable to type 'Iterable<string, any, any>'.",
      fileName: 'src/index.ts',
      line: 15,
      column: 14,
    },
    {
      message: "Non-abstract class 'MissingGeneric' does not implement inherited abstract member 'handle' from class 'Generic<string>'.",
      fileName: 'src/index.ts',
      line: 54,
      column: 8,
    },
    { message: "Cannot find name 'Iterator'.", fileName: '' },
  ]);
});

test('recovers codes from the TypeScript diagnostic table', () => {
  const templates = loadMessageTemplates();
  assert.strictEqual(codeForMessage("Type 'number' is not assignable to type 'string'.", templates), 'TS2322');
  assert.strictEqual(codeForMessage("Cannot find name 'foo'.", templates), 'TS2304');
  assert.strictEqual(codeForMessage("Cannot find name 'foo'. Did you mean 'for'?", templates), 'TS2552');
  assert.strictEqual(codeForMessage('Declarations must be initialized.', templates), null);
});

test('a template with a double space still matches single-spaced text', () => {
  const template = compileTemplate('TS2403', "Subsequent variable declarations must have the same type.  Variable '{0}' must be of type '{1}'.");
  assert.strictEqual(
    codeForMessage("Subsequent variable declarations must have the same type. Variable 'x' must be of type 'string'.", [template]),
    'TS2403',
  );
});

const diag = (fileName: string, line: number, code: string): NormalizedDiagnostic => ({
  source: 'typescript',
  fileName,
  line,
  code,
});

test('scores the file:line:code multiset', () => {
  const baseline = [diag('a.ts', 1, 'TS2322'), diag('a.ts', 1, 'TS2322'), diag('a.ts', 3, 'TS2304')];
  const reported = [diag('a.ts', 1, 'TS2322'), diag('a.ts', 3, 'TS2552'), diag('b.ts', 9, 'TS2322')];
  const score = scoreDiagnostics(baseline, reported);
  assert.strictEqual(score.tp, 1);
  assert.deepStrictEqual(score.fpKeys.sort(), ['a.ts:3:TS2552', 'b.ts:9:TS2322']);
  assert.deepStrictEqual(score.fnKeys.sort(), ['a.ts:1:TS2322', 'a.ts:3:TS2304']);
});

test('the common set drops a target either checker failed', () => {
  const score = (fp: number) => ({
    status: 'ok' as const, ms: 1, reported: fp, tp: 1, fp, fn: 0,
    outOfProgram: 0, unmapped: 0, fpKeys: [], fnKeys: [], unmappedMessages: [],
  });
  const results: TargetResult[] = [
    { name: 'a', tier: 'fixture', tsconfig: 'a', baselineStatus: 'ok', baseline: 1, scores: { 'surge-ts': score(0), 'bolt-ts': score(2) } },
    { name: 'b', tier: 'fixture', tsconfig: 'b', baselineStatus: 'ok', baseline: 1, scores: { 'surge-ts': score(5), 'bolt-ts': { ...score(0), status: 'crash' } } },
  ];
  assert.strictEqual(aggregate(results, 'surge-ts', true).fp, 0);
  assert.strictEqual(aggregate(results, 'surge-ts', false).fp, 5);
  assert.strictEqual(aggregate(results, 'bolt-ts', false).crashed, 1);
});

test('maps one-to-one bolt-ts rewordings', () => {
  const templates = loadMessageTemplates();
  assert.strictEqual(codeForMessage("Property 'b' is missing.", templates), 'TS2741');
  assert.strictEqual(codeForMessage("Generic type 'Box' requires 1 type argument.", templates), 'TS2314');
});

test('renders a chart with a failure pill for a checker that crashed', () => {
  const score = { status: 'ok' as const, ms: 1, reported: 1, tp: 1, fp: 0, fn: 0, outOfProgram: 0, unmapped: 0, fpKeys: [], fnKeys: [], unmappedMessages: [] };
  const results: TargetResult[] = [
    { name: 'a', tier: 'fixture', tsconfig: 'a', baselineStatus: 'ok', baseline: 1, scores: { 'surge-ts': score, 'bolt-ts': score } },
    { name: 'b', tier: 'fixture', tsconfig: 'b', baselineStatus: 'ok', baseline: 1, scores: { 'surge-ts': score, 'bolt-ts': { ...score, status: 'crash' } } },
  ];
  const svg = renderCheckerSvg(results, { date: '2026-09-18T00:00:00.000Z', tsgo: '7.0.2', platform: 'test' });
  assert.ok(svg.startsWith('<svg'));
  assert.ok(svg.includes('1 crash'));
  assert.ok(svg.includes('1 targets both completed'));
});

const fixture = (name: string) =>
  path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../tests/compat-projects', name, 'tsconfig.json');

test('hands bolt-ts the program root files and effective options', () => {
  const { config } = translateConfigForBolt(fixture('binding-pattern-parameters-basic'));
  assert.deepStrictEqual(config.include.map((file) => path.basename(file)), ['index.ts']);
  assert.strictEqual(config.compilerOptions.strictNullChecks, true);
  assert.strictEqual(config.compilerOptions.target, 'ES2025');
});

test('respells lib names the way bolt-ts deserializes them', () => {
  const { config } = translateConfigForBolt(fixture('default-lib-lib-option-dom-basic'));
  assert.ok((config.compilerOptions.lib as string[]).includes('dom'));
});

test('a process past the resident-memory cap is killed and flagged', async () => {
  const hog = 'const keep = []; setInterval(() => keep.push(Buffer.alloc(64 * 1024 * 1024, 1)), 10);';
  const output = await runProcess(process.execPath, ['-e', hog], {
    cwd: process.cwd(),
    timeoutMs: 20_000,
    maxRssBytes: 256 * 1024 * 1024,
  });
  assert.strictEqual(output.memoryExceeded, true);
  assert.strictEqual(output.timedOut, false);
  assert.ok(output.ms < 10_000, `took ${output.ms}ms`);
});
