import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { expect, test } from 'vitest';

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
    surgeTs: { total: 1, byCode: [], byFileCode: [], byFileCodeLine: [] },
    matches: {
      byCode: [],
      onlyTypeScript: [],
      onlySurgeTs: [],
      byFileCode: [],
      onlyTypeScriptFileCode: [],
      onlySurgeTsFileCode: [],
      byFileCodeLine: [],
      onlyTypeScriptFileCodeLine: [],
      onlySurgeTsFileCodeLine: [],
    },
    messageParity: { comparedLocations: 1, matches: 1, mismatches: [] },
    summary: {
      byCodeMatch: true,
      byFileCodeMatch: true,
      byFileCodeLineMatch: true,
      messageMatch: true,
    },
    tooling: { typescriptVersion: 'x', typescriptCommand: 'tsc', surgeTsCommand: 'cargo' },
    details: {
      onlyTypeScript: { rawDiagnosticFingerprints: [] },
      onlySurgeTs: { rawDiagnosticFingerprints: [] },
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
  expect(args.all).toBe(true);
  expect(args.filters).toStrictEqual(['node-protocol', 'reference-types']);
  expect(args.excludes).toStrictEqual(['diagnostics-pack']);
  expect(args.projects).toStrictEqual(['a/tsconfig.json']);
  expect(args.files).toStrictEqual(['b.ts']);
  expect(args.discover).toStrictEqual(['tests/compat-projects']);
  expect(args.maxDiagnostics).toBe(200);
  expect(args.jobs).toBe(4);
});

test('parseSweepArgs rejects unknown flags and bad numbers', () => {
  expect(() => parseSweepArgs(['--nope'])).toThrow(/unknown argument/);
  expect(() => parseSweepArgs(['--jobs', '0'])).toThrow(/positive integer/);
  expect(() => parseSweepArgs(['--project'])).toThrow(/requires a value/);
});

test('selectTargets with no criteria selects nothing', () => {
  const selection = selectTargets({ ...baseArgs }, [presetTarget('a')], [], []);
  expect(selection.hasCriteria).toBe(false);
  expect(selection.selected).toStrictEqual([]);
});

test('--all selects every preset in registry order', () => {
  const presets = presetTargets();
  const selection = selectTargets({ ...baseArgs, all: true }, presets, [], []);
  expect(selection.selected.map((target) => target.name)).toStrictEqual(listPresetNames());
});

test('--filter selects matching presets without --all', () => {
  const presets = [presetTarget('node-protocol-fs'), presetTarget('reference-types'), presetTarget('node-protocol-buf')];
  const selection = selectTargets({ ...baseArgs, filters: ['node-protocol'] }, presets, [], []);
  expect(selection.selected.map((t) => t.name)).toStrictEqual(
    ['node-protocol-fs', 'node-protocol-buf'],
  );
});

test('--exclude moves matching targets to skipped', () => {
  const presets = [presetTarget('alpha'), presetTarget('diagnostics-pack'), presetTarget('beta')];
  const selection = selectTargets({ ...baseArgs, all: true, excludes: ['diagnostics-pack'] }, presets, [], []);
  expect(selection.selected.map((t) => t.name)).toStrictEqual(['alpha', 'beta']);
  expect(selection.skipped.map((t) => t.name)).toStrictEqual(['diagnostics-pack']);
});

test('explicit projects are included even without --all and survive --filter', () => {
  const explicit = [projectTarget('local/app/tsconfig.json')];
  const selection = selectTargets({ ...baseArgs, filters: ['node-protocol'], projects: ['x'] }, [presetTarget('node-protocol-a')], explicit, []);
  expect(selection.selected.map((t) => t.name).sort()).toStrictEqual(
    ['local/app/tsconfig.json', 'node-protocol-a'],
  );
  expect(selection.hasCriteria).toBe(true);
});

test('--list with explicit sources does not pull in the whole registry', () => {
  const selection = selectTargets({ ...baseArgs, list: true, projects: ['x'] }, presetTargets(), [projectTarget('p')], []);
  expect(selection.selected.map((t) => t.name)).toStrictEqual(['p']);
});

test('explicit project alone is a valid selection criterion', () => {
  const selection = selectTargets({ ...baseArgs, projects: ['x'] }, presetTargets(), [projectTarget('p')], []);
  expect(selection.hasCriteria).toBe(true);
  expect(selection.selected.map((t) => t.name)).toStrictEqual(['p']);
});

test('discovered targets are filtered by --filter but explicit ones are not', () => {
  const discovered = [projectTarget('pkg/keep-node-protocol/tsconfig.json'), projectTarget('pkg/drop/tsconfig.json')];
  const selection = selectTargets({ ...baseArgs, discover: ['pkg'], filters: ['node-protocol'] }, [], [], discovered);
  expect(selection.selected.map((t) => t.name)).toStrictEqual(
    ['pkg/keep-node-protocol/tsconfig.json'],
  );
});

test('dedupeTargets keeps the first target per resolved path', () => {
  const targets = [
    presetTarget('preset-x'),
    { name: 'dup', kind: 'project', value: 'dup', resolvedPath: '/abs/preset-x/tsconfig.json' } as SweepTarget,
    projectTarget('unique'),
  ];
  expect(dedupeTargets(targets).map((t) => t.name)).toStrictEqual(['preset-x', 'unique']);
});

test('selectTargets dedupes an explicit project that matches a selected preset', () => {
  const preset = presetTarget('shared');
  const explicit: SweepTarget = { name: 'shared-explicit', kind: 'project', value: 'p', resolvedPath: preset.resolvedPath };
  const selection = selectTargets({ ...baseArgs, all: true, projects: ['p'] }, [preset], [explicit], []);
  expect(selection.selected.map((t) => t.name)).toStrictEqual(['shared']);
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
  expect(basenames).toStrictEqual(['pkg-a', 'pkg-b']);
  expect(targets.every((t) => t.kind === 'project')).toBeTruthy();
});

test('discoverProjectTargets throws on a missing directory', () => {
  expect(() => discoverProjectTargets('/no/such/dir/at/all')).toThrow(/existing directory/);
});

test('deriveResult fails on code-count mismatch and counts surplus', () => {
  const comparison = makeComparison({
    summary: { byCodeMatch: false, byFileCodeMatch: true, byFileCodeLineMatch: true, messageMatch: true } as never,
    matches: {
      onlyTypeScript: [{ key: 'TS2322', typescript: 4, surgeTs: 0 }],
      onlySurgeTs: [],
      byCode: [],
      byFileCode: [],
      onlyTypeScriptFileCode: [],
      onlySurgeTsFileCode: [],
      byFileCodeLine: [],
      onlyTypeScriptFileCodeLine: [],
      onlySurgeTsFileCodeLine: [],
    } as never,
  });
  const result = deriveResult(DEMO, comparison, 10, baseArgs);
  expect(result.passed).toBe(false);
  expect(result.codeCountMatch).toBe(false);
  expect(result.onlyTsc).toBe(4);
});

test('message drift passes by default but fails under --strictMessages', () => {
  const comparison = makeComparison({
    summary: { byCodeMatch: true, byFileCodeMatch: true, byFileCodeLineMatch: true, messageMatch: false } as never,
  });
  expect(deriveResult(DEMO, comparison, 1, baseArgs).passed).toBe(true);
  expect(
    deriveResult(DEMO, comparison, 1, { ...baseArgs, strictMessages: true }).passed,
  ).toBe(false);
});

test('span drift detected from column differences and gated by --strictSpans', () => {
  const comparison = makeComparison({
    details: {
      onlyTypeScript: { rawDiagnosticFingerprints: [{ fileName: 'a.ts', code: 'TS1', line: 3, column: 5, message: 'm', count: 1 }] },
      onlySurgeTs: { rawDiagnosticFingerprints: [{ fileName: 'a.ts', code: 'TS1', line: 3, column: 9, message: 'm', count: 1 }] },
    } as never,
  });
  expect(deriveSpanMatch(comparison)).toBe(false);
  expect(deriveResult(DEMO, comparison, 1, baseArgs).passed).toBe(true);
  expect(deriveResult(DEMO, comparison, 1, { ...baseArgs, strictSpans: true }).passed).toBe(false);
});

test('same-column message difference is not span drift', () => {
  const comparison = makeComparison({
    details: {
      onlyTypeScript: { rawDiagnosticFingerprints: [{ fileName: 'a.ts', code: 'TS1', line: 3, column: 5, message: 'x', count: 1 }] },
      onlySurgeTs: { rawDiagnosticFingerprints: [{ fileName: 'a.ts', code: 'TS1', line: 3, column: 5, message: 'y', count: 1 }] },
    } as never,
  });
  expect(deriveSpanMatch(comparison)).toBe(true);
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
  expect(summary.total).toBe(2);
  expect(summary.passed).toBe(1);
  expect(summary.failed).toBe(1);
  expect(summary.skipped).toBe(1);
  expect(summary.codeCountMismatches).toBe(1);
  expect(summary.exitCode).toBe(1);
});

test('formatPresetLine matches the compact shape', () => {
  const result = deriveResult(presetTarget('node-protocol-buffer-basic'), makeComparison(), 312, baseArgs);
  expect(formatPresetLine(result)).toBe(
    'PASS node-protocol-buffer-basic ts=1 rust=1 onlyTsc=0 onlyRust=0 fileCodeLine=yes message=yes span=yes elapsed=312ms',
  );
});

test('result object exposes the documented keys', () => {
  const result = deriveResult(DEMO, makeComparison(), 5, baseArgs);
  expect(Object.keys(result).sort()).toStrictEqual([
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
