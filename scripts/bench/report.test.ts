import { expect, test } from 'vitest';

import {
  normalizeBenchReport,
  renderBenchmarkSvg,
  renderBenchmarkHtml,
  speedupVsTsc,
  memoryRatioVsTsc,
  formatSpeedup,
  formatMemoryRatio,
  formatBytes,
  hasMemoryData,
  niceAxisScale,
  toolDisplayLabel,
  type BenchReportDocument,
  type BenchReportResult,
} from './report.js';

const MB = 1024 * 1024;

const sampleResult: BenchReportResult = {
  project: 'sample-project',
  rustJobs: 4,
  stats: {
    'tsc': { median: 10, min: 9.5, max: 10.5, runs: 5 },
    'tsgo': { median: 5, min: 4.8, max: 5.4, runs: 5 },
    'surge-ts': { median: 2, min: 1.9, max: 2.2, runs: 5 },
  },
  memory: {
    'tsc': { medianBytes: 2048 * MB, minBytes: 2000 * MB, maxBytes: 2100 * MB, runs: 5, source: 'phys_footprint' },
    'tsgo': { medianBytes: 1024 * MB, minBytes: 1000 * MB, maxBytes: 1100 * MB, runs: 5, source: 'phys_footprint' },
    'surge-ts': { medianBytes: 512 * MB, minBytes: 500 * MB, maxBytes: 550 * MB, runs: 5, source: 'phys_footprint' },
  },
  drift: {
    'tsc': 'known delta vs tsgo',
    'tsgo': 'baseline',
    'surge-ts': 'exact vs tsgo',
  },
};

const timeOnlyResult: BenchReportResult = {
  project: 'time-only',
  rustJobs: 1,
  stats: { 'tsc': { median: 1, min: 1, max: 1, runs: 1 } },
  drift: { 'tsc': 'known delta vs tsgo' },
};

const sampleDoc: BenchReportDocument = {
  meta: {
    timestamp: '2026-07-17T00:00:00.000Z',
    gitCommit: 'abc1234',
    gitBranch: 'main',
    platform: 'darwin arm64',
    cpu: 'Test CPU',
    cores: 8,
    nodeVersion: 'v22.0.0',
    iterations: 5,
    warmup: 1,
    tscVersion: '6.0.3',
    tsgoVersion: '7.0.2',
  },
  results: [sampleResult],
};

test('normalizeBenchReport accepts legacy array shape', () => {
  const doc = normalizeBenchReport([sampleResult]);
  expect(doc.meta).toBe(undefined);
  expect(doc.results.length).toBe(1);
  expect(doc.results[0].project).toBe('sample-project');
});

test('normalizeBenchReport accepts document shape', () => {
  const doc = normalizeBenchReport(sampleDoc);
  expect(doc.meta?.gitCommit).toBe('abc1234');
  expect(doc.results.length).toBe(1);
});

test('normalizeBenchReport rejects unknown shapes', () => {
  expect(() => normalizeBenchReport({ foo: 'bar' })).toThrow();
  expect(() => normalizeBenchReport('nope')).toThrow();
});

test('speedupVsTsc computes ratio against the tsc median', () => {
  expect(speedupVsTsc(sampleResult, 'surge-ts')).toBe(5);
  expect(speedupVsTsc(sampleResult, 'tsgo')).toBe(2);
  expect(speedupVsTsc(sampleResult, 'tsc')).toBe(null);
  const noBaseline: BenchReportResult = { ...sampleResult, stats: { 'surge-ts': sampleResult.stats['surge-ts'] }, drift: {} };
  expect(speedupVsTsc(noBaseline, 'surge-ts')).toBe(null);
});

test('formatSpeedup uses fewer digits for large ratios', () => {
  expect(formatSpeedup(5)).toBe('5.00× vs tsc');
  expect(formatSpeedup(12.34)).toBe('12.3× vs tsc');
});

test('toolDisplayLabel marks the TypeScript major version per tool', () => {
  expect(toolDisplayLabel('tsc')).toBe('tsc (TS 6)');
  expect(toolDisplayLabel('tsgo')).toBe('tsgo (TS 7)');
  expect(toolDisplayLabel('tsgo-singleThreaded')).toBe('tsgo-singleThreaded (TS 7)');
  expect(toolDisplayLabel('surge-ts')).toBe('surge-ts');
});

test('niceAxisScale rounds up to clean tick steps', () => {
  const scale = niceAxisScale(9.7);
  expect(scale.max >= 9.7).toBeTruthy();
  expect(scale.max % scale.step).toBe(0);
  const tiny = niceAxisScale(0);
  expect(tiny.max > 0 && tiny.step > 0).toBeTruthy();
});

test('SVG report includes bars, speedups, drift, and metadata', () => {
  const svg = renderBenchmarkSvg(sampleDoc);
  expect(svg.startsWith('<svg'), 'renders an SVG document').toBeTruthy();
  expect(svg, 'includes the project name').toContain('sample-project');
  expect(svg, 'labels the Rust job count').toContain('jobs=4');
  expect(svg, 'labels tsc as TypeScript 6').toContain('tsc (TS 6)');
  expect(svg, 'labels tsgo as TypeScript 7').toContain('tsgo (TS 7)');
  expect(svg, 'includes the tsc version in the header').toContain('tsc@6.0.3');
  expect(svg, 'includes the tsgo version in the header').toContain('tsgo@7.0.2');
  expect(svg, 'includes the speedup vs tsc').toContain('5.00× vs tsc');
  expect(svg, 'includes the drift status').toContain('exact vs tsgo');
  expect(svg, 'includes the git commit').toContain('abc1234');
  expect(svg, 'includes the CPU model').toContain('Test CPU');
  expect(svg, 'includes the footer disclaimer').toContain('Local-machine-relative');
});

test('SVG report escapes markup in project names', () => {
  const doc: BenchReportDocument = {
    results: [{ ...sampleResult, project: '<script>alert(1)</script>' }],
  };
  const svg = renderBenchmarkSvg(doc);
  expect(svg, 'raw markup must be escaped').not.toContain('<script>');
  expect(svg, 'escaped markup is rendered').toContain('&lt;script&gt;');
});

test('SVG report renders legacy array input without metadata', () => {
  const svg = renderBenchmarkSvg([sampleResult]);
  expect(svg.startsWith('<svg')).toBeTruthy();
  expect(svg).toContain('sample-project');
});

test('HTML report embeds the SVG plus a stats table and metadata', () => {
  const html = renderBenchmarkHtml(sampleDoc);
  expect(html, 'embeds the SVG chart').toContain('<svg');
  expect(html, 'keeps the disclaimer').toContain('local-machine-relative');
  expect(html, 'includes the stats table section').toContain('Detailed results');
  expect(html, 'includes stats table headers').toContain('<th class="num">Median</th>');
  expect(html, 'includes the tsc median').toContain('10.00s');
  expect(html, 'includes the git commit').toContain('abc1234');
  expect(html, 'includes the node version').toContain('v22.0.0');
  expect(html, 'includes iteration counts').toContain('5 (+1 warmup)');
  expect(
    html,
    'names the tsc speed reference version',
  ).toContain('tsc 6.0.3 (TS 6 speed reference)');
  expect(
    html,
    'names the tsgo baseline version',
  ).toContain('tsgo 7.0.2 (TS 7 diagnostic baseline)');
});

test('memoryRatioVsTsc computes ratio against the tsc peak RSS', () => {
  expect(memoryRatioVsTsc(sampleResult, 'surge-ts')).toBe(0.25);
  expect(memoryRatioVsTsc(sampleResult, 'tsgo')).toBe(0.5);
  expect(memoryRatioVsTsc(sampleResult, 'tsc')).toBe(null);
  expect(memoryRatioVsTsc(timeOnlyResult, 'surge-ts')).toBe(null);
});

test('formatBytes picks MB or GB by magnitude', () => {
  expect(formatBytes(512 * MB)).toBe('512 MB');
  expect(formatBytes(2048 * MB)).toBe('2.00 GB');
  expect(formatBytes(1.5 * MB)).toBe('1.5 MB');
});

test('formatMemoryRatio labels the tsc baseline', () => {
  expect(formatMemoryRatio(0.25)).toBe('0.25× of tsc');
  expect(formatMemoryRatio(12.3)).toBe('12.3× of tsc');
});

test('hasMemoryData detects the presence of RSS samples', () => {
  expect(hasMemoryData([sampleResult])).toBe(true);
  expect(hasMemoryData([timeOnlyResult])).toBe(false);
  expect(hasMemoryData([])).toBe(false);
});

test('combined SVG stacks a wall-time panel and a memory panel', () => {
  const svg = renderBenchmarkSvg(sampleDoc);
  expect(svg, 'includes the wall-time panel title').toContain('WALL TIME');
  expect(svg, 'includes the memory panel title').toContain('PEAK MEMORY');
  expect(svg, 'includes the tsc peak RSS').toContain('2.00 GB');
  expect(svg, 'includes the memory ratio vs tsc').toContain('0.25× of tsc');
});

test('SVG panel selection renders only the requested panel', () => {
  const timeSvg = renderBenchmarkSvg(sampleDoc, 'time');
  expect(timeSvg).toContain('WALL TIME');
  expect(timeSvg).not.toContain('PEAK MEMORY');

  const memorySvg = renderBenchmarkSvg(sampleDoc, 'memory');
  expect(memorySvg).toContain('PEAK MEMORY');
  expect(memorySvg).not.toContain('WALL TIME');
});

test('SVG omits the memory panel when no RSS was sampled', () => {
  const svg = renderBenchmarkSvg([timeOnlyResult]);
  expect(svg).toContain('WALL TIME');
  expect(svg).not.toContain('PEAK MEMORY');
});

test('HTML report renders tabs when memory data is present', () => {
  const html = renderBenchmarkHtml(sampleDoc);
  expect(html, 'has the wall-time tab').toContain('id="tab-time"');
  expect(html, 'has the memory tab').toContain('id="tab-memory"');
  expect(html, 'labels the memory tab').toContain('Peak memory');
  expect(html, 'includes the memory table').toContain('<th class="num">Median peak memory</th>');
  expect(html, 'includes the memory source').toContain('phys_footprint');
});

test('HTML report skips tabs when only timing data exists', () => {
  const html = renderBenchmarkHtml([timeOnlyResult]);
  expect(html, 'no memory tab without RSS data').not.toContain('id="tab-memory"');
  expect(html, 'still renders the timing table').toContain('Detailed results');
});
