import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import type { ComparisonResult } from './compare-tsc';
import {
  buildSummary,
  dedupeTargets,
  deriveResult,
  deriveSpanMatch,
  discoverProjectTargets,
  formatPresetLine,
  listPresetNames,
  parseSweepArgs,
  presetTargets,
  selectTargets,
  type SweepArgs,
  type SweepTarget,
} from './sweep-presets';

const baseArgs: SweepArgs = {
  all: false,
  filters: [],
  excludes: [],
  projects: [],
  files: [],
  discover: [],
  list: false,
  json: false,
  verbose: false,
  strictMessages: false,
  strictSpans: false,
};

function presetTarget(name: string): SweepTarget {
  return { name, kind: 'preset', value: name, resolvedPath: `/abs/${name}/tsconfig.json` };
}

function projectTarget(name: string, resolvedPath = `/abs/${name}`): SweepTarget {
  return { name, kind: 'project', value: name, resolvedPath };
}

function makeComparison(overrides: Partial<ComparisonResult> = {}): ComparisonResult {
  const base = {
    mode: 'project',
    project: 'demo',
    file: null,
    typescript: { total: 1, byCode: [], byFileCode: [], byFileCodeLine: [] },
    typescriptRust: { total: 1, byCode: [], byFileCode: [], byFileCodeLine: [] },
    matches: {
      byCode: [],
      onlyTypeScript: [],
      onlyTypeScriptRust: [],
      byFileCode: [],
      onlyTypeScriptFileCode: [],
      onlyTypeScriptRustFileCode: [],
      byFileCodeLine: [],
      onlyTypeScriptFileCodeLine: [],
      onlyTypeScriptRustFileCodeLine: [],
    },
    messageParity: { comparedLocations: 1, matches: 1, mismatches: [] },
    summary: {
      byCodeMatch: true,
      byFileCodeMatch: true,
      byFileCodeLineMatch: true,
      messageMatch: true,
    },
    tooling: { typescriptVersion: 'x', typescriptCommand: 'tsc', typescriptRustCommand: 'cargo' },
    details: {
      onlyTypeScript: { rawDiagnosticFingerprints: [] },
      onlyTypeScriptRust: { rawDiagnosticFingerprints: [] },
    },
  } as unknown as ComparisonResult;

  return {
    ...base,
    ...overrides,
    summary: { ...base.summary, ...(overrides.summary ?? {}) },
  } as ComparisonResult;
}

const DEMO = presetTarget('demo');

test('parseSweepArgs collects flags, repeated filters, and target sources', () => {
  const args = parseSweepArgs([
    '--all',
    '--filter',
    'node-protocol',
    '--filter',
    'reference-types',
    '--exclude',
    'diagnostics-pack',
    '--project',
    'a/tsconfig.json',
    '--file',
    'b.ts',
    '--discover',
    'tests/compat-projects',
    '--maxDiagnostics',
    '200',
    '--jobs',
    '4',
    '--json',
    '--strictMessages',
    '--strictSpans',
  ]);
  assert.equal(args.all, true);
  assert.deepEqual(args.filters, ['node-protocol', 'reference-types']);
  assert.deepEqual(args.excludes, ['diagnostics-pack']);
  assert.deepEqual(args.projects, ['a/tsconfig.json']);
  assert.deepEqual(args.files, ['b.ts']);
  assert.deepEqual(args.discover, ['tests/compat-projects']);
  assert.equal(args.maxDiagnostics, 200);
  assert.equal(args.jobs, 4);
});

test('parseSweepArgs rejects unknown flags and bad numbers', () => {
  assert.throws(() => parseSweepArgs(['--nope']), /unknown argument/);
  assert.throws(() => parseSweepArgs(['--jobs', '0']), /positive integer/);
  assert.throws(() => parseSweepArgs(['--project']), /requires a value/);
});

test('selectTargets with no criteria selects nothing', () => {
  const selection = selectTargets({ ...baseArgs }, [presetTarget('a')], [], []);
  assert.equal(selection.hasCriteria, false);
  assert.deepEqual(selection.selected, []);
});

test('--all selects every preset in registry order', () => {
  const presets = presetTargets();
  const selection = selectTargets({ ...baseArgs, all: true }, presets, [], []);
  assert.deepEqual(
    selection.selected.map((target) => target.name),
    listPresetNames(),
  );
});

test('--filter selects matching presets without --all', () => {
  const presets = [presetTarget('node-protocol-fs'), presetTarget('reference-types'), presetTarget('node-protocol-buf')];
  const selection = selectTargets({ ...baseArgs, filters: ['node-protocol'] }, presets, [], []);
  assert.deepEqual(
    selection.selected.map((t) => t.name),
    ['node-protocol-fs', 'node-protocol-buf'],
  );
});

test('--exclude moves matching targets to skipped', () => {
  const presets = [presetTarget('alpha'), presetTarget('diagnostics-pack'), presetTarget('beta')];
  const selection = selectTargets({ ...baseArgs, all: true, excludes: ['diagnostics-pack'] }, presets, [], []);
  assert.deepEqual(selection.selected.map((t) => t.name), ['alpha', 'beta']);
  assert.deepEqual(selection.skipped.map((t) => t.name), ['diagnostics-pack']);
});

test('explicit projects are included even without --all and survive --filter', () => {
  const explicit = [projectTarget('local/app/tsconfig.json')];
  const selection = selectTargets({ ...baseArgs, filters: ['node-protocol'], projects: ['x'] }, [presetTarget('node-protocol-a')], explicit, []);
  assert.deepEqual(
    selection.selected.map((t) => t.name).sort(),
    ['local/app/tsconfig.json', 'node-protocol-a'],
  );
  assert.equal(selection.hasCriteria, true);
});

test('--list with explicit sources does not pull in the whole registry', () => {
  const selection = selectTargets({ ...baseArgs, list: true, projects: ['x'] }, presetTargets(), [projectTarget('p')], []);
  assert.deepEqual(selection.selected.map((t) => t.name), ['p']);
});

test('explicit project alone is a valid selection criterion', () => {
  const selection = selectTargets({ ...baseArgs, projects: ['x'] }, presetTargets(), [projectTarget('p')], []);
  assert.equal(selection.hasCriteria, true);
  assert.deepEqual(selection.selected.map((t) => t.name), ['p']);
});

test('discovered targets are filtered by --filter but explicit ones are not', () => {
  const discovered = [projectTarget('pkg/keep-node-protocol/tsconfig.json'), projectTarget('pkg/drop/tsconfig.json')];
  const selection = selectTargets({ ...baseArgs, discover: ['pkg'], filters: ['node-protocol'] }, [], [], discovered);
  assert.deepEqual(selection.selected.map((t) => t.name), ['pkg/keep-node-protocol/tsconfig.json']);
});

test('dedupeTargets keeps the first target per resolved path', () => {
  const targets = [
    presetTarget('preset-x'),
    { name: 'dup', kind: 'project', value: 'dup', resolvedPath: '/abs/preset-x/tsconfig.json' } as SweepTarget,
    projectTarget('unique'),
  ];
  assert.deepEqual(dedupeTargets(targets).map((t) => t.name), ['preset-x', 'unique']);
});

test('selectTargets dedupes an explicit project that matches a selected preset', () => {
  const preset = presetTarget('shared');
  const explicit: SweepTarget = { name: 'shared-explicit', kind: 'project', value: 'p', resolvedPath: preset.resolvedPath };
  const selection = selectTargets({ ...baseArgs, all: true, projects: ['p'] }, [preset], [explicit], []);
  assert.deepEqual(selection.selected.map((t) => t.name), ['shared']);
});

test('discoverProjectTargets walks a directory and skips node_modules', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'sweep-discover-'));
  fs.mkdirSync(path.join(root, 'pkg-a'), { recursive: true });
  fs.mkdirSync(path.join(root, 'nested', 'pkg-b'), { recursive: true });
  fs.mkdirSync(path.join(root, 'node_modules', 'dep'), { recursive: true });
  fs.writeFileSync(path.join(root, 'pkg-a', 'tsconfig.json'), '{}');
  fs.writeFileSync(path.join(root, 'nested', 'pkg-b', 'tsconfig.json'), '{}');
  fs.writeFileSync(path.join(root, 'node_modules', 'dep', 'tsconfig.json'), '{}');

  const targets = discoverProjectTargets(root);
  const basenames = targets.map((t) => path.basename(path.dirname(t.resolvedPath))).sort();
  assert.deepEqual(basenames, ['pkg-a', 'pkg-b']);
  assert.ok(targets.every((t) => t.kind === 'project'));
});

test('discoverProjectTargets throws on a missing directory', () => {
  assert.throws(() => discoverProjectTargets('/no/such/dir/at/all'), /existing directory/);
});

test('deriveResult fails on code-count mismatch and counts surplus', () => {
  const comparison = makeComparison({
    summary: { byCodeMatch: false, byFileCodeMatch: true, byFileCodeLineMatch: true, messageMatch: true } as never,
    matches: {
      onlyTypeScript: [{ key: 'TS2322', typescript: 4, typescriptRust: 0 }],
      onlyTypeScriptRust: [],
      byCode: [],
      byFileCode: [],
      onlyTypeScriptFileCode: [],
      onlyTypeScriptRustFileCode: [],
      byFileCodeLine: [],
      onlyTypeScriptFileCodeLine: [],
      onlyTypeScriptRustFileCodeLine: [],
    } as never,
  });
  const result = deriveResult(DEMO, comparison, 10, baseArgs);
  assert.equal(result.passed, false);
  assert.equal(result.codeCountMatch, false);
  assert.equal(result.onlyTsc, 4);
});

test('message drift passes by default but fails under --strictMessages', () => {
  const comparison = makeComparison({
    summary: { byCodeMatch: true, byFileCodeMatch: true, byFileCodeLineMatch: true, messageMatch: false } as never,
  });
  assert.equal(deriveResult(DEMO, comparison, 1, baseArgs).passed, true);
  assert.equal(deriveResult(DEMO, comparison, 1, { ...baseArgs, strictMessages: true }).passed, false);
});

test('span drift detected from column differences and gated by --strictSpans', () => {
  const comparison = makeComparison({
    details: {
      onlyTypeScript: { rawDiagnosticFingerprints: [{ fileName: 'a.ts', code: 'TS1', line: 3, column: 5, message: 'm', count: 1 }] },
      onlyTypeScriptRust: { rawDiagnosticFingerprints: [{ fileName: 'a.ts', code: 'TS1', line: 3, column: 9, message: 'm', count: 1 }] },
    } as never,
  });
  assert.equal(deriveSpanMatch(comparison), false);
  assert.equal(deriveResult(DEMO, comparison, 1, baseArgs).passed, true);
  assert.equal(deriveResult(DEMO, comparison, 1, { ...baseArgs, strictSpans: true }).passed, false);
});

test('same-column message difference is not span drift', () => {
  const comparison = makeComparison({
    details: {
      onlyTypeScript: { rawDiagnosticFingerprints: [{ fileName: 'a.ts', code: 'TS1', line: 3, column: 5, message: 'x', count: 1 }] },
      onlyTypeScriptRust: { rawDiagnosticFingerprints: [{ fileName: 'a.ts', code: 'TS1', line: 3, column: 5, message: 'y', count: 1 }] },
    } as never,
  });
  assert.equal(deriveSpanMatch(comparison), true);
});

test('buildSummary aggregates counts and exit code', () => {
  const results = [
    deriveResult(presetTarget('a'), makeComparison(), 100, baseArgs),
    deriveResult(
      presetTarget('b'),
      makeComparison({ summary: { byCodeMatch: false, byFileCodeMatch: true, byFileCodeLineMatch: true, messageMatch: true } as never }),
      200,
      baseArgs,
    ),
  ];
  const summary = buildSummary(results, [presetTarget('skipped-one')], 1234);
  assert.equal(summary.total, 2);
  assert.equal(summary.passed, 1);
  assert.equal(summary.failed, 1);
  assert.equal(summary.skipped, 1);
  assert.equal(summary.codeCountMismatches, 1);
  assert.equal(summary.exitCode, 1);
});

test('formatPresetLine matches the compact shape', () => {
  const result = deriveResult(presetTarget('node-protocol-buffer-basic'), makeComparison(), 312, baseArgs);
  assert.equal(
    formatPresetLine(result),
    'PASS node-protocol-buffer-basic ts=1 rust=1 onlyTsc=0 onlyRust=0 fileCodeLine=yes message=yes span=yes elapsed=312ms',
  );
});

test('result object exposes the documented keys', () => {
  const result = deriveResult(DEMO, makeComparison(), 5, baseArgs);
  assert.deepEqual(Object.keys(result).sort(), [
    'codeCountMatch',
    'elapsedMs',
    'fileCodeLineMatch',
    'kind',
    'messageMatch',
    'onlyRust',
    'onlyTsc',
    'passed',
    'preset',
    'rustDiagnostics',
    'spanMatch',
    'typescriptDiagnostics',
  ]);
});
