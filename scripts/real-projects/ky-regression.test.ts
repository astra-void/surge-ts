import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

import { compareProject } from '../oracle/compare-tsc';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const kyTsconfig = path.join(repoRoot, '.local-projects', 'ky', 'tsconfig.json');
const typescriptLib = path.join(repoRoot, 'node_modules', 'typescript', 'lib', 'lib.es5.d.ts');

// Regression gate: `ky` (sindresorhus/ky) is a `tsc`-reports-0 project, so every
// surge-ts diagnostic on it is a false positive. This locks the 2026-06-20 0/0
// parity. The project is not vendored (`.local-projects` is gitignored) and the
// oracle needs the `typescript` package, so the gate skips when either is absent
// — mirroring the physical-lib rust tests. When both are present, any drift from
// tsc fails the gate.
test('ky matches tsc at 0/0 (real-project regression gate)', (t) => {
  if (!fs.existsSync(kyTsconfig)) {
    t.skip('ky project not present at .local-projects/ky (gitignored, not vendored)');
    return;
  }
  if (!fs.existsSync(typescriptLib)) {
    t.skip('typescript package not installed (oracle baseline unavailable)');
    return;
  }

  const result = compareProject(kyTsconfig, 'ky', 500, false, undefined);

  assert.equal(
    result.typescript.total,
    0,
    'precondition: tsc must still report 0 diagnostics on ky',
  );
  assert.equal(
    result.surgeTs.total,
    0,
    `surge-ts must report 0 diagnostics on ky; only-surge fingerprints: ${JSON.stringify(
      result.matches.onlySurgeTs,
    )}`,
  );
  assert.ok(
    result.summary.byCodeMatch && result.summary.byFileCodeMatch,
    'surge-ts must match tsc by code and file/code on ky',
  );
});
