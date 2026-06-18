import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import assert from 'node:assert';

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
  assert.strictEqual(result.status, 0, `Script failed: ${result.stderr}\n${result.stdout}`);
  assert.ok((result.stdout || '').includes('Performance:'), 'Should output performance table');
  assert.ok((result.stdout || '').includes('tsgo'), 'Should include tsgo in the benchmark output when it is installed');
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
  assert.notStrictEqual(result.status, 0, 'Script should fail when ignoreDeprecations is used');
  assert.ok((result.stderr || '').includes('ignoreDeprecations'), 'Should mention ignoreDeprecations in error');
});

test('bench script generates scale fixture correctly', () => {
  const result = spawnSync(packageManagerExecutable, [...packageManagerArgsPrefix, benchScript, '--generate', 'test-scale', '--files', '2', '--symbols', '2', '--iterations', '1', '--warmup', '0'], {
    cwd: workspaceRoot,
    encoding: 'utf8',
    shell: process.platform === 'win32',
  });

  if (result.error) throw result.error;
  assert.strictEqual(result.status, 0, `Generate scale fixture failed: ${result.stderr}\n${result.stdout}`);
  assert.ok(existsSync(path.join(workspaceRoot, '.bench/generated/test-scale/tsconfig.json')));
});

test('bench script generates json output', () => {
  const tempJson = path.join(workspaceRoot, '.bench', 'test-output.json');
  
  const result = spawnSync(packageManagerExecutable, [...packageManagerArgsPrefix, benchScript, '--preset', 'current', '--iterations', '1', '--warmup', '0', '--json', tempJson, '--rustJobs', '4'], {
    cwd: workspaceRoot,
    encoding: 'utf8',
    shell: process.platform === 'win32',
  });

  if (result.error) throw result.error;
  assert.strictEqual(result.status, 0, `Script failed: ${result.stderr}\n${result.stdout}`);
  assert.ok(existsSync(tempJson), 'Should create JSON file');
  const data = JSON.parse(readFileSync(tempJson, 'utf8'));
  assert.ok(Array.isArray(data), 'JSON should be an array of results');
  assert.ok(data.some((entry: { rustJobs?: number }) => entry.rustJobs === 4), 'JSON should include the Rust job count');
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
  assert.strictEqual(result.status, 0, `Script failed: ${result.stderr}\n${result.stdout}`);
  
  assert.ok(existsSync(tempChart), 'Should create SVG chart');
  const chartContent = readFileSync(tempChart, 'utf8');
  assert.ok(chartContent.includes('<svg'), 'Chart should contain SVG tag');
  assert.ok(chartContent.includes('jobs=4'), 'Chart should label the Rust job count');
  
  assert.ok(existsSync(tempHtml), 'Should create HTML file');
  const htmlContent = readFileSync(tempHtml, 'utf8');
  assert.ok(htmlContent.includes('<svg'), 'HTML should embed SVG tag');
  assert.ok(htmlContent.includes('jobs=4'), 'HTML should label the Rust job count');
  assert.ok(htmlContent.includes('local-machine-relative'), 'HTML should contain disclaimer');
});
