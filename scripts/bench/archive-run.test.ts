import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import assert from 'node:assert';

import {
  parseArgs,
  sanitizeLabel,
  timestampSlug,
  defaultOutDir,
  buildPlan,
  extractBenchMedians,
  extractAuthKitCounts,
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
  assert.strictEqual(slug, '2026-06-16T12-30-15');
  assert.ok(!/[:.]/.test(slug), 'slug should not contain colons or dots');
});

test('defaultOutDir nests under .bench/runs/<timestamp>', () => {
  const dir = defaultOutDir('/repo', '2026-06-16T12-30-15');
  assert.strictEqual(dir, path.join('/repo', '.bench', 'runs', '2026-06-16T12-30-15'));
});

test('sanitizeLabel strips unsafe characters', () => {
  assert.strictEqual(sanitizeLabel('builtin-removal-before'), 'builtin-removal-before');
  assert.strictEqual(sanitizeLabel('feature/auth kit!!'), 'feature-auth-kit');
  assert.strictEqual(sanitizeLabel('  ../../etc/passwd  '), 'etc-passwd');
  assert.strictEqual(sanitizeLabel('--weird--'), 'weird');
});

test('parseArgs defaults are all off and parses flags', () => {
  const empty = parseArgs([]);
  assert.deepStrictEqual(empty, {
    bench: false,
    realAuthKit: false,
    label: null,
    out: null,
    dryRun: false,
  });

  const parsed = parseArgs(['--', '--bench', '--real-auth-kit', '--label', 'x', '--out', 'p', '--dryRun']);
  assert.strictEqual(parsed.bench, true);
  assert.strictEqual(parsed.realAuthKit, true);
  assert.strictEqual(parsed.label, 'x');
  assert.strictEqual(parsed.out, 'p');
  assert.strictEqual(parsed.dryRun, true);
});

test('parseArgs rejects unknown flags', () => {
  assert.throws(() => parseArgs(['--nope']), /Unknown argument/);
});

test('buildPlan emits only requested steps with bench JSON output', () => {
  const benchOnly = buildPlan({ bench: true, realAuthKit: false }, '/out');
  assert.strictEqual(benchOnly.length, 1);
  assert.strictEqual(benchOnly[0].name, 'bench-compilers');
  assert.ok(benchOnly[0].argv.includes('bench:compilers'));
  assert.strictEqual(benchOnly[0].jsonFile, path.join('/out', 'bench-compilers.json'));

  const both = buildPlan({ bench: true, realAuthKit: true }, '/out');
  assert.deepStrictEqual(both.map((s) => s.name), ['bench-compilers', 'real-auth-kit']);
  const real = both[1];
  assert.ok(real.argv.includes('real:auth-kit'));
  assert.strictEqual(real.jsonFile, null);
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
  assert.strictEqual(medians.length, 1);
  assert.strictEqual(medians[0].project, 'demo');
  assert.strictEqual(medians[0].medians.tsc, 1.23);
  assert.strictEqual(medians[0].medians['surge-ts'], 0.21);
  assert.strictEqual(medians[0].medians.tsgo, null);
});

test('extractBenchMedians handles non-array input gracefully', () => {
  assert.deepStrictEqual(extractBenchMedians(null), []);
  assert.deepStrictEqual(extractBenchMedians({}), []);
});

test('extractAuthKitCounts parses measurement markdown', () => {
  const md = [
    '## Raw Totals',
    '- TypeScript total diagnostics: 12',
    '- surge-ts total diagnostics: 12',
    '- code-count match: yes',
  ].join('\n');
  const counts = extractAuthKitCounts(md);
  assert.ok(counts);
  assert.strictEqual(counts?.typescriptTotal, 12);
  assert.strictEqual(counts?.surgeTsTotal, 12);
  assert.strictEqual(counts?.codeCountMatch, true);

  assert.strictEqual(extractAuthKitCounts('nothing relevant here'), null);
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
    authKit: null,
    parseWarnings: [],
  });

  assert.strictEqual(summary.timestamp, '2026-06-16T12-30-15');
  assert.strictEqual(summary.label, 'test-run');
  assert.strictEqual(summary.git.branch, 'main');
  assert.strictEqual(summary.commands.length, 1);
  assert.strictEqual(summary.commands[0].ok, true);
  assert.strictEqual(summary.medians[0].project, 'demo');

  const roundTripped = JSON.parse(JSON.stringify(summary));
  assert.deepStrictEqual(roundTripped, summary);
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
      authKit: null,
      parseWarnings: ['Bench JSON was not produced despite a successful run.'],
    }),
  );

  assert.ok(md.includes('# Benchmark Archive — 2026-06-16T12-30-15 — after'));
  assert.ok(md.includes('| fail |') || md.includes('fail'), 'should report fail status');
  assert.ok(md.includes('dirty'));
  assert.ok(md.includes('Parse Notes'));
});

test('dry run prints commands and writes nothing, exits zero', () => {
  const result = spawnSync(
    packageManagerExecutable,
    [...packageManagerArgsPrefix, archiveScript, '--bench', '--real-auth-kit', '--label', 'demo', '--dryRun'],
    { cwd: workspaceRoot, encoding: 'utf8', shell: process.platform === 'win32' },
  );

  if (result.error) throw result.error;
  assert.strictEqual(result.status, 0, `Dry run failed: ${result.stderr}\n${result.stdout}`);
  const stdout = result.stdout || '';
  assert.ok(stdout.includes('Output directory:'), 'should print output directory');
  assert.ok(stdout.includes('bench:compilers'), 'should print bench command');
  assert.ok(stdout.includes('real:auth-kit'), 'should print real-auth-kit command');
  assert.ok(stdout.includes('Dry run'), 'should note dry run');
});
