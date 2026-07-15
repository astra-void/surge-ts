import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';

import {
  ALLOCATORS,
  cargoBuildArgs,
  countDiagnosticsFromJson,
  generateSyntheticProject,
  median,
  parseBenchArgs,
  renderSummaryMarkdown,
  summarize,
  type RunRecord,
} from './allocator-bench.js';

function record(overrides: Partial<RunRecord>): RunRecord {
  return {
    allocator: 'system',
    scenario: 'medium-jobs-1',
    project: 'tsconfig.json',
    jobs: 1,
    iteration: 0,
    wallTimeMs: 100,
    peakRssBytes: 1024,
    peakRssSource: 'macos-time',
    finalRssBytes: null,
    finalRssSource: 'unavailable',
    exitCode: 0,
    fileCount: 10,
    workerCountRequested: 1,
    diagnosticCount: 0,
    ...overrides,
  };
}

test('median of odd, even, and empty runs', () => {
  assert.equal(median([3, 1, 2]), 2);
  assert.equal(median([4, 1, 3, 2]), 2.5);
  assert.equal(median([]), null);
});

test('cargoBuildArgs: system build has no feature flags', () => {
  assert.deepEqual(cargoBuildArgs('system'), ['build', '--release', '-p', 'surge-ts-cli']);
  assert.deepEqual(cargoBuildArgs('mimalloc'), [
    'build',
    '--release',
    '-p',
    'surge-ts-cli',
    '--features',
    'mimalloc',
  ]);
});

test('summarize computes per-scenario/allocator medians and worst peak RSS', () => {
  const records = [
    record({ wallTimeMs: 100, peakRssBytes: 10 }),
    record({ wallTimeMs: 300, peakRssBytes: 30, iteration: 1 }),
    record({ wallTimeMs: 200, peakRssBytes: 20, iteration: 2 }),
    record({ allocator: 'mimalloc', wallTimeMs: 50, peakRssBytes: null }),
  ];
  const summaries = summarize(records);
  assert.equal(summaries.length, 2);

  const system = summaries.find((s) => s.allocator === 'system')!;
  assert.equal(system.runs, 3);
  assert.equal(system.medianWallTimeMs, 200);
  assert.equal(system.medianPeakRssBytes, 20);
  assert.equal(system.worstPeakRssBytes, 30);

  const mimalloc = summaries.find((s) => s.allocator === 'mimalloc')!;
  assert.equal(mimalloc.medianPeakRssBytes, null);
  assert.equal(mimalloc.worstPeakRssBytes, null);
});

test('renderSummaryMarkdown emits one row per scenario/allocator pair', () => {
  const markdown = renderSummaryMarkdown(
    summarize([record({}), record({ allocator: 'jemalloc' })]),
  );
  const rows = markdown.split('\n');
  assert.equal(rows.length, 4);
  assert.match(rows[2], /jemalloc/);
  assert.match(rows[3], /system/);
});

test('countDiagnosticsFromJson parses surge --format json output', () => {
  assert.equal(countDiagnosticsFromJson('{"diagnostics": [{}, {}]}'), 2);
  assert.equal(countDiagnosticsFromJson('{"diagnostics": []}'), 0);
  assert.equal(countDiagnosticsFromJson('not json'), null);
  assert.equal(countDiagnosticsFromJson('{}'), null);
});

test('parseBenchArgs defaults and validation', () => {
  const parsed = parseBenchArgs([]);
  assert.deepEqual(parsed.allocators, [...ALLOCATORS]);
  assert.equal(parsed.iterations, 5);
  assert.equal(parsed.warmup, 1);
  assert.equal(parsed.skipBuild, false);

  const custom = parseBenchArgs([
    '--allocators',
    'system,mimalloc',
    '--iterations',
    '7',
    '--skipBuild',
    '--scenario',
    'medium',
  ]);
  assert.deepEqual(custom.allocators, ['system', 'mimalloc']);
  assert.equal(custom.iterations, 7);
  assert.equal(custom.skipBuild, true);
  assert.equal(custom.scenarioFilter, 'medium');

  assert.equal(parseBenchArgs(['--', '--iterations', '3']).iterations, 3);

  assert.throws(() => parseBenchArgs(['--allocators', 'tcmalloc']), /unknown allocator/);
  assert.throws(() => parseBenchArgs(['--iterations', '0']), /positive integer/);
  assert.throws(() => parseBenchArgs(['--bogus']), /unknown argument/);
});

test('generateSyntheticProject writes a deterministic self-contained fixture', () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'surge-alloc-bench-'));
  try {
    const tsconfig = generateSyntheticProject(dir, 5);
    assert.equal(tsconfig, path.join(dir, 'tsconfig.json'));
    const config = JSON.parse(readFileSync(tsconfig, 'utf8'));
    assert.equal(config.compilerOptions.noEmit, true);

    const files = readdirSync(path.join(dir, 'src')).sort();
    assert.equal(files.length, 6); // 5 leaves + index.ts
    assert.ok(files.includes('index.ts'));

    const index = readFileSync(path.join(dir, 'src', 'index.ts'), 'utf8');
    assert.match(index, /import { value4 } from "\.\/file_4";/);

    const again = generateSyntheticProject(dir, 5);
    assert.equal(readFileSync(again, 'utf8'), readFileSync(tsconfig, 'utf8'));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
