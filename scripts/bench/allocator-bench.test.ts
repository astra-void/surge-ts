import { mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { expect, test } from 'vitest';

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
  expect(median([3, 1, 2])).toBe(2);
  expect(median([4, 1, 3, 2])).toBe(2.5);
  expect(median([])).toBe(null);
});

test('cargoBuildArgs: system build has no feature flags', () => {
  expect(cargoBuildArgs('system')).toStrictEqual(['build', '--release', '-p', 'surge-ts-cli']);
  expect(cargoBuildArgs('mimalloc')).toStrictEqual([
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
  expect(summaries.length).toBe(2);

  const system = summaries.find((s) => s.allocator === 'system')!;
  expect(system.runs).toBe(3);
  expect(system.medianWallTimeMs).toBe(200);
  expect(system.medianPeakRssBytes).toBe(20);
  expect(system.worstPeakRssBytes).toBe(30);

  const mimalloc = summaries.find((s) => s.allocator === 'mimalloc')!;
  expect(mimalloc.medianPeakRssBytes).toBe(null);
  expect(mimalloc.worstPeakRssBytes).toBe(null);
});

test('renderSummaryMarkdown emits one row per scenario/allocator pair', () => {
  const markdown = renderSummaryMarkdown(
    summarize([record({}), record({ allocator: 'jemalloc' })]),
  );
  const rows = markdown.split('\n');
  expect(rows.length).toBe(4);
  expect(rows[2]).toMatch(/jemalloc/);
  expect(rows[3]).toMatch(/system/);
});

test('countDiagnosticsFromJson parses surge --format json output', () => {
  expect(countDiagnosticsFromJson('{"diagnostics": [{}, {}]}')).toBe(2);
  expect(countDiagnosticsFromJson('{"diagnostics": []}')).toBe(0);
  expect(countDiagnosticsFromJson('not json')).toBe(null);
  expect(countDiagnosticsFromJson('{}')).toBe(null);
});

test('parseBenchArgs defaults and validation', () => {
  const parsed = parseBenchArgs([]);
  expect(parsed.allocators).toStrictEqual([...ALLOCATORS]);
  expect(parsed.iterations).toBe(5);
  expect(parsed.warmup).toBe(1);
  expect(parsed.skipBuild).toBe(false);

  const custom = parseBenchArgs([
    '--allocators',
    'system,mimalloc',
    '--iterations',
    '7',
    '--skipBuild',
    '--scenario',
    'medium',
  ]);
  expect(custom.allocators).toStrictEqual(['system', 'mimalloc']);
  expect(custom.iterations).toBe(7);
  expect(custom.skipBuild).toBe(true);
  expect(custom.scenarioFilter).toBe('medium');

  expect(parseBenchArgs(['--', '--iterations', '3']).iterations).toBe(3);

  expect(() => parseBenchArgs(['--allocators', 'tcmalloc'])).toThrow(/unknown allocator/);
  expect(() => parseBenchArgs(['--iterations', '0'])).toThrow(/positive integer/);
  expect(() => parseBenchArgs(['--bogus'])).toThrow(/unknown argument/);
});

test('generateSyntheticProject writes a deterministic self-contained fixture', () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'surge-alloc-bench-'));
  try {
    const tsconfig = generateSyntheticProject(dir, 5);
    expect(tsconfig).toBe(path.join(dir, 'tsconfig.json'));
    const config = JSON.parse(readFileSync(tsconfig, 'utf8'));
    expect(config.compilerOptions.noEmit).toBe(true);

    const files = readdirSync(path.join(dir, 'src')).sort();
    expect(files.length).toBe(6); // 5 leaves + index.ts
    expect(files).toContain('index.ts');

    const index = readFileSync(path.join(dir, 'src', 'index.ts'), 'utf8');
    expect(index).toMatch(/import { value4 } from "\.\/file_4";/);

    const again = generateSyntheticProject(dir, 5);
    expect(readFileSync(again, 'utf8')).toBe(readFileSync(tsconfig, 'utf8'));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
