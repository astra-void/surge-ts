import test from 'node:test';
import assert from 'node:assert';

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
    'tsc': 'baseline',
    'tsgo': 'known delta',
    'surge-ts': 'exact vs tsc',
  },
};

const timeOnlyResult: BenchReportResult = {
  project: 'time-only',
  rustJobs: 1,
  stats: { 'tsc': { median: 1, min: 1, max: 1, runs: 1 } },
  drift: { 'tsc': 'baseline' },
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
  assert.strictEqual(doc.meta, undefined);
  assert.strictEqual(doc.results.length, 1);
  assert.strictEqual(doc.results[0].project, 'sample-project');
});

test('normalizeBenchReport accepts document shape', () => {
  const doc = normalizeBenchReport(sampleDoc);
  assert.strictEqual(doc.meta?.gitCommit, 'abc1234');
  assert.strictEqual(doc.results.length, 1);
});

test('normalizeBenchReport rejects unknown shapes', () => {
  assert.throws(() => normalizeBenchReport({ foo: 'bar' }));
  assert.throws(() => normalizeBenchReport('nope'));
});

test('speedupVsTsc computes ratio against the tsc median', () => {
  assert.strictEqual(speedupVsTsc(sampleResult, 'surge-ts'), 5);
  assert.strictEqual(speedupVsTsc(sampleResult, 'tsgo'), 2);
  assert.strictEqual(speedupVsTsc(sampleResult, 'tsc'), null);
  const noBaseline: BenchReportResult = { ...sampleResult, stats: { 'surge-ts': sampleResult.stats['surge-ts'] }, drift: {} };
  assert.strictEqual(speedupVsTsc(noBaseline, 'surge-ts'), null);
});

test('formatSpeedup uses fewer digits for large ratios', () => {
  assert.strictEqual(formatSpeedup(5), '5.00× vs tsc');
  assert.strictEqual(formatSpeedup(12.34), '12.3× vs tsc');
});

test('toolDisplayLabel marks the TypeScript major version per tool', () => {
  assert.strictEqual(toolDisplayLabel('tsc'), 'tsc (TS 6)');
  assert.strictEqual(toolDisplayLabel('tsgo'), 'tsgo (TS 7)');
  assert.strictEqual(toolDisplayLabel('tsgo-singleThreaded'), 'tsgo-singleThreaded (TS 7)');
  assert.strictEqual(toolDisplayLabel('surge-ts'), 'surge-ts');
});

test('niceAxisScale rounds up to clean tick steps', () => {
  const scale = niceAxisScale(9.7);
  assert.ok(scale.max >= 9.7);
  assert.strictEqual(scale.max % scale.step, 0);
  const tiny = niceAxisScale(0);
  assert.ok(tiny.max > 0 && tiny.step > 0);
});

test('SVG report includes bars, speedups, drift, and metadata', () => {
  const svg = renderBenchmarkSvg(sampleDoc);
  assert.ok(svg.startsWith('<svg'), 'renders an SVG document');
  assert.ok(svg.includes('sample-project'), 'includes the project name');
  assert.ok(svg.includes('jobs=4'), 'labels the Rust job count');
  assert.ok(svg.includes('tsc (TS 6)'), 'labels tsc as TypeScript 6');
  assert.ok(svg.includes('tsgo (TS 7)'), 'labels tsgo as TypeScript 7');
  assert.ok(svg.includes('tsc@6.0.3'), 'includes the tsc version in the header');
  assert.ok(svg.includes('tsgo@7.0.2'), 'includes the tsgo version in the header');
  assert.ok(svg.includes('5.00× vs tsc'), 'includes the speedup vs tsc');
  assert.ok(svg.includes('exact vs tsc'), 'includes the drift status');
  assert.ok(svg.includes('abc1234'), 'includes the git commit');
  assert.ok(svg.includes('Test CPU'), 'includes the CPU model');
  assert.ok(svg.includes('Local-machine-relative'), 'includes the footer disclaimer');
});

test('SVG report escapes markup in project names', () => {
  const doc: BenchReportDocument = {
    results: [{ ...sampleResult, project: '<script>alert(1)</script>' }],
  };
  const svg = renderBenchmarkSvg(doc);
  assert.ok(!svg.includes('<script>'), 'raw markup must be escaped');
  assert.ok(svg.includes('&lt;script&gt;'), 'escaped markup is rendered');
});

test('SVG report renders legacy array input without metadata', () => {
  const svg = renderBenchmarkSvg([sampleResult]);
  assert.ok(svg.startsWith('<svg'));
  assert.ok(svg.includes('sample-project'));
});

test('HTML report embeds the SVG plus a stats table and metadata', () => {
  const html = renderBenchmarkHtml(sampleDoc);
  assert.ok(html.includes('<svg'), 'embeds the SVG chart');
  assert.ok(html.includes('local-machine-relative'), 'keeps the disclaimer');
  assert.ok(html.includes('Detailed results'), 'includes the stats table section');
  assert.ok(html.includes('<th class="num">Median</th>'), 'includes stats table headers');
  assert.ok(html.includes('10.00s'), 'includes the tsc median');
  assert.ok(html.includes('abc1234'), 'includes the git commit');
  assert.ok(html.includes('v22.0.0'), 'includes the node version');
  assert.ok(html.includes('5 (+1 warmup)'), 'includes iteration counts');
  assert.ok(html.includes('tsc 6.0.3 (TS 6 baseline)'), 'names the tsc baseline version');
  assert.ok(html.includes('tsgo 7.0.2 (TS 7 native)'), 'names the tsgo version');
});

test('memoryRatioVsTsc computes ratio against the tsc peak RSS', () => {
  assert.strictEqual(memoryRatioVsTsc(sampleResult, 'surge-ts'), 0.25);
  assert.strictEqual(memoryRatioVsTsc(sampleResult, 'tsgo'), 0.5);
  assert.strictEqual(memoryRatioVsTsc(sampleResult, 'tsc'), null);
  assert.strictEqual(memoryRatioVsTsc(timeOnlyResult, 'surge-ts'), null);
});

test('formatBytes picks MB or GB by magnitude', () => {
  assert.strictEqual(formatBytes(512 * MB), '512 MB');
  assert.strictEqual(formatBytes(2048 * MB), '2.00 GB');
  assert.strictEqual(formatBytes(1.5 * MB), '1.5 MB');
});

test('formatMemoryRatio labels the tsc baseline', () => {
  assert.strictEqual(formatMemoryRatio(0.25), '0.25× of tsc');
  assert.strictEqual(formatMemoryRatio(12.3), '12.3× of tsc');
});

test('hasMemoryData detects the presence of RSS samples', () => {
  assert.strictEqual(hasMemoryData([sampleResult]), true);
  assert.strictEqual(hasMemoryData([timeOnlyResult]), false);
  assert.strictEqual(hasMemoryData([]), false);
});

test('combined SVG stacks a wall-time panel and a memory panel', () => {
  const svg = renderBenchmarkSvg(sampleDoc);
  assert.ok(svg.includes('WALL TIME'), 'includes the wall-time panel title');
  assert.ok(svg.includes('PEAK MEMORY'), 'includes the memory panel title');
  assert.ok(svg.includes('2.00 GB'), 'includes the tsc peak RSS');
  assert.ok(svg.includes('0.25× of tsc'), 'includes the memory ratio vs tsc');
});

test('SVG panel selection renders only the requested panel', () => {
  const timeSvg = renderBenchmarkSvg(sampleDoc, 'time');
  assert.ok(timeSvg.includes('WALL TIME'));
  assert.ok(!timeSvg.includes('PEAK MEMORY'));

  const memorySvg = renderBenchmarkSvg(sampleDoc, 'memory');
  assert.ok(memorySvg.includes('PEAK MEMORY'));
  assert.ok(!memorySvg.includes('WALL TIME'));
});

test('SVG omits the memory panel when no RSS was sampled', () => {
  const svg = renderBenchmarkSvg([timeOnlyResult]);
  assert.ok(svg.includes('WALL TIME'));
  assert.ok(!svg.includes('PEAK MEMORY'));
});

test('HTML report renders tabs when memory data is present', () => {
  const html = renderBenchmarkHtml(sampleDoc);
  assert.ok(html.includes('id="tab-time"'), 'has the wall-time tab');
  assert.ok(html.includes('id="tab-memory"'), 'has the memory tab');
  assert.ok(html.includes('Peak memory'), 'labels the memory tab');
  assert.ok(html.includes('<th class="num">Median peak memory</th>'), 'includes the memory table');
  assert.ok(html.includes('phys_footprint'), 'includes the memory source');
});

test('HTML report skips tabs when only timing data exists', () => {
  const html = renderBenchmarkHtml([timeOnlyResult]);
  assert.ok(!html.includes('id="tab-memory"'), 'no memory tab without RSS data');
  assert.ok(html.includes('Detailed results'), 'still renders the timing table');
});
