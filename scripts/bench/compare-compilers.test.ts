import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { expect, test } from 'vitest';

const scriptPath = fileURLToPath(import.meta.url);
const scriptDir = path.dirname(scriptPath);
const workspaceRoot = path.resolve(scriptDir, '../..');
const benchScript = path.join(scriptDir, 'compare-compilers.ts');

const packageManagerExecutable = process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm';
const packageManagerArgsPrefix = ['exec', 'tsx'];

test('bench script parsing and basic run', () => {
  const result = spawnSync(packageManagerExecutable, [...packageManagerArgsPrefix, benchScript, '--preset', 'current', '--iterations', '1', '--warmup', '0', '--rustJobs', '1'], {
    cwd: workspaceRoot,
    encoding: 'utf8',
    shell: process.platform === 'win32',
  });
  
  if (result.error) throw result.error;
  expect(result.status, `Script failed: ${result.stderr}\n${result.stdout}`).toBe(0);
  expect(result.stdout || '', 'Should output performance table').toContain('Performance:');
  expect(
    result.stdout || '',
    'Should include tsgo in the benchmark output when it is installed',
  ).toContain('tsgo');
});

test('bench script rejects ignoreDeprecations', () => {
  const tempFixtureDir = path.join(workspaceRoot, '.bench', 'test-fixture');
  const tempFixturePath = path.join(tempFixtureDir, 'tsconfig.json');
  
  mkdirSync(tempFixtureDir, { recursive: true });
  writeFileSync(tempFixturePath, JSON.stringify({ compilerOptions: { ignoreDeprecations: "6.0" } }));

  const result = spawnSync(packageManagerExecutable, [...packageManagerArgsPrefix, benchScript, '--project', tempFixturePath, '--iterations', '1', '--warmup', '0'], {
    cwd: workspaceRoot,
    encoding: 'utf8',
    shell: process.platform === 'win32',
  });

  if (result.error) throw result.error;
  expect(result.status, 'Script should fail when ignoreDeprecations is used').not.toBe(0);
  expect(
    result.stderr || '',
    'Should mention ignoreDeprecations in error',
  ).toContain('ignoreDeprecations');
});

test('bench script generates scale fixture correctly', () => {
  const result = spawnSync(packageManagerExecutable, [...packageManagerArgsPrefix, benchScript, '--generate', 'test-scale', '--files', '2', '--symbols', '2', '--iterations', '1', '--warmup', '0'], {
    cwd: workspaceRoot,
    encoding: 'utf8',
    shell: process.platform === 'win32',
  });

  if (result.error) throw result.error;
  expect(
    result.status,
    `Generate scale fixture failed: ${result.stderr}\n${result.stdout}`,
  ).toBe(0);
  expect(
    existsSync(path.join(workspaceRoot, '.bench/generated/test-scale/tsconfig.json')),
  ).toBeTruthy();
});

test('bench script generates json output', () => {
  const tempJson = path.join(workspaceRoot, '.bench', 'test-output.json');
  
  const result = spawnSync(packageManagerExecutable, [...packageManagerArgsPrefix, benchScript, '--preset', 'current', '--iterations', '1', '--warmup', '0', '--json', tempJson, '--rustJobs', '4'], {
    cwd: workspaceRoot,
    encoding: 'utf8',
    shell: process.platform === 'win32',
  });

  if (result.error) throw result.error;
  expect(result.status, `Script failed: ${result.stderr}\n${result.stdout}`).toBe(0);
  expect(existsSync(tempJson), 'Should create JSON file').toBeTruthy();
  const data = JSON.parse(readFileSync(tempJson, 'utf8'));
  expect(Array.isArray(data.results), 'JSON should contain a results array').toBeTruthy();
  expect(
    data.results.some((entry: { rustJobs?: number }) => entry.rustJobs === 4),
    'JSON should include the Rust job count',
  ).toBeTruthy();
  expect(
    typeof data.meta?.timestamp === 'string',
    'JSON should record the run timestamp',
  ).toBeTruthy();
  expect(typeof data.meta?.platform === 'string', 'JSON should record the platform').toBeTruthy();
  expect(data.meta?.iterations, 'JSON should record the iteration count').toBe(1);
  expect(data.meta?.tscVersion ?? '', 'tsc speed reference should be TypeScript 6').toMatch(/^6\./);
  expect(data.meta?.tsgoVersion ?? '', 'tsgo should be TypeScript 7').toMatch(/^7\./);
  const first = data.results[0];
  if (first.drift?.tsgo && first.drift.tsgo !== 'skipped') {
    expect(first.drift.tsgo, 'tsgo (TS 7) should be the diagnostic baseline').toBe('baseline');
    expect(
      first.drift['surge-ts'] ?? '',
      'surge-ts drift should be measured against tsgo, not the TS 6 reference',
    ).toMatch(/vs tsgo$/);
  }
  expect(
    first.memory && typeof first.memory === 'object',
    'results should include a memory record',
  ).toBeTruthy();
  if (process.platform === 'darwin' || process.platform === 'linux') {
    expect(
      first.memory.tsc && first.memory.tsc.medianBytes > 0,
      'tsc peak memory should be sampled',
    ).toBeTruthy();
    expect(
      first.memory['surge-ts'] && first.memory['surge-ts'].medianBytes > 0,
      'surge-ts peak memory should be sampled',
    ).toBeTruthy();
  }
  if (process.platform === 'darwin') {
    expect(first.memory.tsc.source, 'macOS should measure phys_footprint').toBe('phys_footprint');
  }
});

test('bench script fromJson generates chart and html', () => {
  const tempJson = path.join(workspaceRoot, '.bench', 'test-output.json');
  const tempChart = path.join(workspaceRoot, '.bench', 'test-output.svg');
  const tempHtml = path.join(workspaceRoot, '.bench', 'test-output.html');
  
  // Create dummy JSON if not exists from previous test
  if (!existsSync(tempJson)) {
    writeFileSync(tempJson, JSON.stringify([{
      project: "dummy",
      rustJobs: 4,
      stats: {
        tsc: { median: 1, min: 1, max: 1, runs: 1 },
        'surge-ts': { median: 1, min: 1, max: 1, runs: 1 }
      },
      drift: { tsc: "baseline", 'surge-ts': "exact vs tsc" }
    }]));
  }

  const result = spawnSync(packageManagerExecutable, [...packageManagerArgsPrefix, benchScript, '--fromJson', tempJson, '--chart', tempChart, '--html', tempHtml], {
    cwd: workspaceRoot,
    encoding: 'utf8',
    shell: process.platform === 'win32',
  });

  if (result.error) throw result.error;
  expect(result.status, `Script failed: ${result.stderr}\n${result.stdout}`).toBe(0);
  
  expect(existsSync(tempChart), 'Should create SVG chart').toBeTruthy();
  const chartContent = readFileSync(tempChart, 'utf8');
  expect(chartContent, 'Chart should contain SVG tag').toContain('<svg');
  expect(chartContent, 'Chart should label the Rust job count').toContain('jobs=4');
  
  expect(existsSync(tempHtml), 'Should create HTML file').toBeTruthy();
  const htmlContent = readFileSync(tempHtml, 'utf8');
  expect(htmlContent, 'HTML should embed SVG tag').toContain('<svg');
  expect(htmlContent, 'HTML should label the Rust job count').toContain('jobs=4');
  expect(htmlContent, 'HTML should contain disclaimer').toContain('local-machine-relative');
});

test('bench script fromJson accepts legacy array-shaped JSON', () => {
  const legacyJson = path.join(workspaceRoot, '.bench', 'test-output-legacy.json');
  const legacyChart = path.join(workspaceRoot, '.bench', 'test-output-legacy.svg');

  writeFileSync(legacyJson, JSON.stringify([{
    project: "legacy-dummy",
    rustJobs: 2,
    stats: {
      tsc: { median: 2, min: 2, max: 2, runs: 1 },
      'surge-ts': { median: 1, min: 1, max: 1, runs: 1 }
    },
    drift: { tsc: "baseline", 'surge-ts': "exact vs tsc" }
  }]));

  const result = spawnSync(packageManagerExecutable, [...packageManagerArgsPrefix, benchScript, '--fromJson', legacyJson, '--chart', legacyChart], {
    cwd: workspaceRoot,
    encoding: 'utf8',
    shell: process.platform === 'win32',
  });

  if (result.error) throw result.error;
  expect(result.status, `Script failed: ${result.stderr}\n${result.stdout}`).toBe(0);
  const chartContent = readFileSync(legacyChart, 'utf8');
  expect(chartContent, 'Chart should render legacy results').toContain('legacy-dummy');
  expect(chartContent, 'Chart should label the Rust job count').toContain('jobs=2');
});
