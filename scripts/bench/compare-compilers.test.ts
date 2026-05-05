import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
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
  const result = spawnSync(packageManagerExecutable, [...packageManagerArgsPrefix, benchScript, '--preset', 'current', '--iterations', '1', '--warmup', '0'], {
    cwd: workspaceRoot,
    encoding: 'utf8',
    shell: process.platform === 'win32',
  });
  
  if (result.error) throw result.error;
  assert.strictEqual(result.status, 0, `Script failed: ${result.stderr}\n${result.stdout}`);
  assert.ok((result.stdout || '').includes('Performance:'), 'Should output performance table');
});

test('bench script rejects ignoreDeprecations', () => {
  const tempFixtureDir = path.join(workspaceRoot, '.bench', 'test-fixture');
  const tempFixturePath = path.join(tempFixtureDir, 'tsconfig.json');
  import('node:fs').then(fs => {
    fs.mkdirSync(tempFixtureDir, { recursive: true });
    fs.writeFileSync(tempFixturePath, JSON.stringify({ compilerOptions: { ignoreDeprecations: "6.0" } }));
  });

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
