import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(import.meta.url);
const scriptDir = path.dirname(scriptPath);
const workspaceRoot = path.resolve(scriptDir, '../..');
const compareScript = path.join(scriptDir, 'compare-tsc.ts');

const bannedTerms = [
  'CategorizedCountEntry',
  'nodeModulesSourceDiagnostics',
  'nodeModulesJavaScriptSourceDiagnostics',
  'node_modules source',
  'dependency JavaScript',
  'source-prefix',
  'candidate',
  'synthetic built-in',
  'ES/lib-lite',
  'Node-like',
  'DOM-like',
  'JSX-like',
  'local-unresolved',
  'simplewebauthn',
  'uuid',
  'react',
  'noble',
  'auth-kit',
];

test('oracle source stays raw and classifier-free', () => {
  const files = [
    path.join(scriptDir, 'compare-tsc.ts'),
    path.join(scriptDir, 'compare-tsc.test.ts'),
    path.join(scriptDir, 'README.md'),
  ];

  const source = files.map((file) => fs.readFileSync(file, 'utf8')).join('\n');

  for (const term of bannedTerms) {
    assert.equal(
      containsWholeTerm(source, term),
      false,
      `oracle source still contains banned term: ${term}`,
    );
  }
});

test('oracle output stays raw on a tiny unresolved project', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'oracle-guard-'));
  const tsconfig = path.join(root, 'tsconfig.json');
  const entry = path.join(root, 'src', 'index.ts');

  fs.mkdirSync(path.dirname(entry), { recursive: true });
  fs.writeFileSync(
    tsconfig,
    JSON.stringify({ compilerOptions: { strict: true, noEmit: true }, include: ['src/**/*.ts'] }),
  );
  fs.writeFileSync(entry, 'import { MissingThing } from "./missing";\nexport const value: MissingThing = UnknownGlobal;\n');

  const result = spawnSync(
    'pnpm',
    ['exec', 'tsx', compareScript, '--project', tsconfig, '--maxDiagnostics', '200'],
    {
      cwd: workspaceRoot,
      encoding: 'utf8',
    },
  );

  assert.equal(result.status, 0, result.stderr || result.stdout);
  const output = `${result.stdout ?? ''}\n${result.stderr ?? ''}`;

  for (const term of bannedTerms) {
    assert.equal(
      containsWholeTerm(output, term),
      false,
      `oracle output still contains banned term: ${term}`,
    );
  }
});

function containsWholeTerm(source: string, term: string): boolean {
  const escaped = term.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const pattern = new RegExp(`(^|[^A-Za-z0-9_])${escaped}([^A-Za-z0-9_]|$)`, 'i');
  return pattern.test(source);
}
