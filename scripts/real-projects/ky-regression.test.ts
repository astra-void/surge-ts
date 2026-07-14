import { spawnSync } from 'node:child_process';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

import { compareProject, resolveSurgeBin } from '../oracle/compare-tsc';
import { resolveTypeScriptLibDir } from '../lib/generate-default-libs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const kyTsconfig = path.join(repoRoot, '.local-projects', 'ky', 'tsconfig.json');
const typescriptLib = (() => {
  try {
    return path.join(resolveTypeScriptLibDir(), 'lib.es5.d.ts');
  } catch {
    return path.join(repoRoot, 'node_modules', 'typescript', 'lib', 'lib.es5.d.ts');
  }
})();

function runSurgeJson(extraArgs: string[]): unknown {
  const result = spawnSync(
    resolveSurgeBin(),
    ['--project', kyTsconfig, '--format', 'json', ...extraArgs],
    { cwd: repoRoot, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 },
  );
  if (result.error) {
    throw new Error(`failed to run surge-ts-cli: ${result.error.message}`);
  }
  return JSON.parse(result.stdout ?? '');
}

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

// Gate the suppression counters behind the 0/0 parity claim. ky's 0/0 holds in
// the tsc profile by *suppressing* `surge::`/lib diagnostics; this asserts that
// none of that suppression is hiding a source-level miss. The native profile
// disables suppression, so any diagnostic it surfaces in a ky *source* file
// (`source/...`, not a `node_modules` lib `.d.ts`) is a regression — in
// particular a recursive-type cycle note degrading a source type, the case the
// suppressed-diagnostics audit flagged. Locked at zero after the 2026-06-20
// cycle-tolerant resolution landing.
test('ky surfaces no suppressed source-level diagnostics (native profile)', (t) => {
  if (!fs.existsSync(kyTsconfig) || !fs.existsSync(typescriptLib)) {
    t.skip('ky project or typescript package not present');
    return;
  }

  const report = runSurgeJson(['--diagnosticProfile', 'native', '--maxDiagnostics', '2000']) as {
    diagnostics: Array<{ code: string; fileName: string }>;
  };
  const sourceLevel = report.diagnostics.filter(
    (diagnostic) => !diagnostic.fileName.includes('node_modules'),
  );
  assert.deepEqual(
    sourceLevel,
    [],
    `ky source files must surface no diagnostics even with suppression off; got: ${JSON.stringify(
      sourceLevel,
    )}`,
  );
});

// Every external (package) reference in ky resolves (the lone
// `@type-challenges/utils` type-only import), so the unresolved-external figure
// must stay zero: a non-zero value would mean a dependency stopped resolving and
// is being silently stubbed, which the externalModuleStubs `total` count alone
// could not distinguish from a benign resolved reference.
test('ky has no unresolved external module stubs', (t) => {
  if (!fs.existsSync(kyTsconfig) || !fs.existsSync(typescriptLib)) {
    t.skip('ky project or typescript package not present');
    return;
  }

  const report = runSurgeJson(['--compatReport']) as {
    externalModuleStubs: { total: number; unresolved: number; resolved: number };
  };
  assert.equal(
    report.externalModuleStubs.unresolved,
    0,
    `ky external references must all resolve; stubs: ${JSON.stringify(
      report.externalModuleStubs,
    )}`,
  );
});
