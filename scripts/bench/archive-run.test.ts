import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { expect, test } from 'vitest';

import {
  parseArgs,
  sanitizeLabel,
  timestampSlug,
  defaultOutDir,
  buildPlan,
  extractBenchMedians,
  buildSummary,
  renderSummaryMarkdown,
} from './archive-run.ts';

const scriptPath = fileURLToPath(import.meta.url);
const scriptDir = path.dirname(scriptPath);
const workspaceRoot = path.resolve(scriptDir, '../..');
const archiveScript = path.join(scriptDir, 'archive-run.ts');

const packageManagerExecutable = process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm';
const packageManagerArgsPrefix = ['exec', 'tsx'];

test('timestampSlug produces a filesystem-safe path segment', () => {
  const slug = timestampSlug(new Date('2026-06-16T12:30:15.123Z'));
  expect(slug).toBe('2026-06-16T12-30-15');
  expect(slug, 'slug should not contain colons or dots').not.toMatch(/[:.]/);
});

test('defaultOutDir nests under .bench/runs/<timestamp>', () => {
  const dir = defaultOutDir('/repo', '2026-06-16T12-30-15');
  expect(dir).toBe(path.join('/repo', '.bench', 'runs', '2026-06-16T12-30-15'));
});

test('sanitizeLabel strips unsafe characters', () => {
  expect(sanitizeLabel('builtin-removal-before')).toBe('builtin-removal-before');
  expect(sanitizeLabel('feature/new perf!!')).toBe('feature-new-perf');
  expect(sanitizeLabel('  ../../etc/passwd  ')).toBe('etc-passwd');
  expect(sanitizeLabel('--weird--')).toBe('weird');
});

test('parseArgs defaults are all off and parses flags', () => {
  const empty = parseArgs([]);
  expect(empty).toStrictEqual({
    bench: false,
    label: null,
    out: null,
    dryRun: false,
  });

  const parsed = parseArgs(['--', '--bench', '--label', 'x', '--out', 'p', '--dryRun']);
  expect(parsed.bench).toBe(true);
  expect(parsed.label).toBe('x');
  expect(parsed.out).toBe('p');
  expect(parsed.dryRun).toBe(true);
});

test('parseArgs rejects unknown flags', () => {
  expect(() => parseArgs(['--nope'])).toThrow(/Unknown argument/);
});

test('buildPlan emits only requested steps with bench JSON output', () => {
  const benchOnly = buildPlan({ bench: true }, '/out');
  expect(benchOnly.length).toBe(1);
  expect(benchOnly[0].name).toBe('bench-compilers');
  expect(benchOnly[0].argv).toContain('bench:compilers');
  expect(benchOnly[0].jsonFile).toBe(path.join('/out', 'bench-compilers.json'));

  expect(buildPlan({ bench: false }, '/out')).toStrictEqual([]);
});

test('extractBenchMedians pulls medians per tool', () => {
  const medians = extractBenchMedians([
    {
      project: 'demo',
      stats: {
        tsc: { median: 1.23, min: 1, max: 2, runs: 5 },
        'surge-ts': { median: 0.21, min: 0, max: 1, runs: 5 },
        tsgo: null,
      },
    },
  ]);
  expect(medians.length).toBe(1);
  expect(medians[0].project).toBe('demo');
  expect(medians[0].medians.tsc).toBe(1.23);
  expect(medians[0].medians['surge-ts']).toBe(0.21);
  expect(medians[0].medians.tsgo).toBe(null);
});

test('extractBenchMedians handles non-array input gracefully', () => {
  expect(extractBenchMedians(null)).toStrictEqual([]);
  expect(extractBenchMedians({})).toStrictEqual([]);
});

test('extractBenchMedians accepts the { meta, results } document shape', () => {
  const medians = extractBenchMedians({
    meta: { timestamp: '2026-07-17T00:00:00.000Z' },
    results: [
      {
        project: 'demo',
        stats: {
          tsc: { median: 1.5, min: 1, max: 2, runs: 3 },
        },
      },
    ],
  });
  expect(medians.length).toBe(1);
  expect(medians[0].project).toBe('demo');
  expect(medians[0].medians.tsc).toBe(1.5);
});

test('buildSummary produces the expected JSON shape', () => {
  const summary = buildSummary({
    timestamp: '2026-06-16T12-30-15',
    label: 'test-run',
    outDir: '/out',
    git: { branch: 'main', commit: 'abc123', dirty: false },
    commands: [
      { name: 'bench-compilers', command: 'pnpm run bench:compilers', exitCode: 0, ok: true, logFile: 'bench.txt', jsonFile: 'bench.json' },
    ],
    medians: [{ project: 'demo', medians: { tsc: 1.0, tsgo: null, 'tsgo-singleThreaded': null, 'surge-ts': 0.2 } }],
    parseWarnings: [],
  });

  expect(summary.timestamp).toBe('2026-06-16T12-30-15');
  expect(summary.label).toBe('test-run');
  expect(summary.git.branch).toBe('main');
  expect(summary.commands.length).toBe(1);
  expect(summary.commands[0].ok).toBe(true);
  expect(summary.medians[0].project).toBe('demo');

  const roundTripped = JSON.parse(JSON.stringify(summary));
  expect(roundTripped).toStrictEqual(summary);
});

test('renderSummaryMarkdown includes label, status, and parse note', () => {
  const md = renderSummaryMarkdown(
    buildSummary({
      timestamp: '2026-06-16T12-30-15',
      label: 'after',
      outDir: '/out',
      git: { branch: 'main', commit: 'abcdef123456', dirty: true },
      commands: [
        { name: 'bench-compilers', command: 'pnpm run bench:compilers', exitCode: 1, ok: false, logFile: 'b.txt', jsonFile: null },
      ],
      medians: [],
        parseWarnings: ['Bench JSON was not produced despite a successful run.'],
    }),
  );

  expect(md).toContain('# Benchmark Archive — 2026-06-16T12-30-15 — after');
  expect(md.includes('| fail |') || md.includes('fail'), 'should report fail status').toBeTruthy();
  expect(md).toContain('dirty');
  expect(md).toContain('Parse Notes');
});

test('dry run prints commands and writes nothing, exits zero', () => {
  const result = spawnSync(
    packageManagerExecutable,
    [...packageManagerArgsPrefix, archiveScript, '--bench', '--label', 'demo', '--dryRun'],
    { cwd: workspaceRoot, encoding: 'utf8', shell: process.platform === 'win32' },
  );

  if (result.error) throw result.error;
  expect(result.status, `Dry run failed: ${result.stderr}\n${result.stdout}`).toBe(0);
  const stdout = result.stdout || '';
  expect(stdout, 'should print output directory').toContain('Output directory:');
  expect(stdout, 'should print bench command').toContain('bench:compilers');
  expect(stdout, 'should note dry run').toContain('Dry run');
});
