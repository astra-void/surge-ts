import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

import { compareProject } from '../oracle/compare-tsc';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const unnamedTsconfig = path.resolve(repoRoot, '../../nextjs/unnamed/tsconfig.json');
const typescriptLib = path.join(repoRoot, 'node_modules', 'typescript', 'lib', 'lib.es5.d.ts');

// The current false-positive watermark (2026-07-02). `unnamed` is a
// tsc-reports-0 project, so every surge diagnostic is an over-report; this
// ceiling only ratchets DOWN — lower it whenever a burn-down pass lands, and
// never raise it to absorb a regression.
const FALSE_POSITIVE_CEILING = 46;

// One comparison serves both gates: the project run takes minutes, so each
// test re-running it would double the gate's wall clock for the same data.
let cachedComparison: ReturnType<typeof compareProject> | undefined;
function unnamedComparison(): ReturnType<typeof compareProject> {
  cachedComparison ??= compareProject(unnamedTsconfig, 'unnamed', 500, false, undefined);
  return cachedComparison;
}

// Regression gate: `unnamed` is the React/Next.js false-positive corpus. Unlike
// ky (an exact 0/0 parity claim), this gate is a count ceiling: it fails when
// the total over-report count rises above the recorded watermark, catching a
// React-typing regression without requiring full parity. The project is local
// and not vendored, and the oracle needs the `typescript` package, so the gate
// skips when either is absent — mirroring the ky gate.
test('unnamed stays at or below the false-positive watermark', (t) => {
  if (!fs.existsSync(unnamedTsconfig)) {
    t.skip('unnamed project not present at ../../nextjs/unnamed (local, not vendored)');
    return;
  }
  if (!fs.existsSync(typescriptLib)) {
    t.skip('typescript package not installed (oracle baseline unavailable)');
    return;
  }

  const result = unnamedComparison();

  assert.equal(
    result.typescript.total,
    0,
    'precondition: tsc must still report 0 diagnostics on unnamed',
  );
  assert.ok(
    result.surgeTs.total <= FALSE_POSITIVE_CEILING,
    `unnamed over-reports rose to ${result.surgeTs.total} (ceiling ${FALSE_POSITIVE_CEILING}); ` +
      `new only-surge fingerprints indicate a regression: ${JSON.stringify(
        result.matches.onlySurgeTs.slice(0, 20),
      )}`,
  );
});

// The clusters cleared by the 2026-07-02 React pass stay cleared: react-hook-form
// render-prop bindings (TS7031) went 9 → 0 via function-type binding-pattern
// parsing and export-shadow scope threading. Any TS7031 on this corpus is a
// regression of that chain, independent of where the total ceiling sits.
test('unnamed reports no implicit-any binding elements (TS7031)', (t) => {
  if (!fs.existsSync(unnamedTsconfig) || !fs.existsSync(typescriptLib)) {
    t.skip('unnamed project or typescript package not present');
    return;
  }

  const result = unnamedComparison();
  const ts7031 = result.matches.onlySurgeTs.filter(
    (fingerprint) => fingerprint.code === 'TS7031',
  );
  assert.deepEqual(
    ts7031,
    [],
    `TS7031 must stay cleared on unnamed; got: ${JSON.stringify(ts7031)}`,
  );
});
