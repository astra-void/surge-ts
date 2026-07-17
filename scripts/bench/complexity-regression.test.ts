import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';

import {
  generateInheritanceProject,
  generateModuleGraphProject,
  generateOverloadProject,
  generateUnionProject,
  MODULE_GRAPH_DEP_COUNT,
} from './complexity-gen.js';
import {
  classifyGrowth,
  evaluateExpectation,
  parseArgs,
  parseTimingsCounters,
  projectSpecs,
  renderMarkdownReport,
  sha256,
  type CounterCaseResult,
} from './complexity-regression.js';

// Captured (abbreviated) from a real `SURGE_TIMINGS=1 surge --project … --jobs 1`
// stderr: the io: section and file_metrics precede the counters block and must
// not be picked up; RSS stages and CLI timings surround it.
const SAMPLE_STDERR = `RSS stages:
  parse_complete  rss=     120.0MB  delta=       n/a  peak=     130.0MB  fp=     125.0MB  fp_peak=     135.0MB  t=   12.000ms
Timings:
  parsing: 10.123ms
  ambient_collection: 0.100ms
  io:
    canonicalize_calls: 999
    canonicalize_cache_hits: 990
    canonicalize_cache_hit_rate: 99.1%
    canonicalize_syscall_time: 0.010ms
  file_metrics:
    src/index.ts | collect_type_declarations=1 lower_type_declarations=2 validate_local_type_declarations=1 | collect_time=0.100ms validate_time=0.050ms
  counters:
    files_total: 12
    type_declaration_table_clone_count: 2
    dependency_declaration_table_clone_count: 0
    generated_default_lib_table_clone_count: 0
    union_type_clone_count: 519
    union_type_payload_alloc_count: 120
    declaration_lookup_layer_count_avg: 1.52
    overload_array_alloc_count: 35
CLI timings:
  total: 42.000ms
`;

test('parseTimingsCounters extracts counters and ignores io/file_metrics/RSS sections', () => {
  const counters = parseTimingsCounters(SAMPLE_STDERR);
  assert.equal(counters.get('files_total'), 12);
  assert.equal(counters.get('type_declaration_table_clone_count'), 2);
  assert.equal(counters.get('dependency_declaration_table_clone_count'), 0);
  assert.equal(counters.get('union_type_clone_count'), 519);
  assert.equal(counters.get('declaration_lookup_layer_count_avg'), 1.52);
  assert.equal(counters.get('overload_array_alloc_count'), 35);
  assert.equal(counters.has('canonicalize_calls'), false);
  assert.equal(counters.has('parsing'), false);
  assert.equal(counters.has('total'), false);
});

test('parseTimingsCounters returns empty map when no counters block exists', () => {
  assert.equal(parseTimingsCounters('Timings:\n  parsing: 1.0ms\n').size, 0);
});

test('classifyGrowth: all-zero series', () => {
  const { classification, tailExponent } = classifyGrowth([64, 128, 256], [0, 0, 0]);
  assert.equal(classification, 'zero');
  assert.equal(tailExponent, null);
});

test('classifyGrowth: constant series', () => {
  const { classification } = classifyGrowth([64, 128, 256], [7, 7, 7]);
  assert.equal(classification, 'constant');
});

test('classifyGrowth: linear series, including fixed offset', () => {
  assert.equal(classifyGrowth([64, 128, 256], [100, 200, 400]).classification, 'linear');
  // 50 fixed + n: the tail exponent converges to 1 despite the offset.
  assert.equal(classifyGrowth([64, 128, 256, 512], [114, 178, 306, 562]).classification, 'linear');
});

test('classifyGrowth: quadratic series is superlinear', () => {
  const sizes = [64, 128, 256];
  const totals = sizes.map((n) => n * n);
  const { classification, tailExponent } = classifyGrowth(sizes, totals);
  assert.equal(classification, 'superlinear');
  assert.ok(tailExponent! > 1.9);
});

test('classifyGrowth rejects mismatched or too-short input', () => {
  assert.throws(() => classifyGrowth([64], [1]));
  assert.throws(() => classifyGrowth([64, 128], [1]));
});

test('evaluateExpectation gates zero and superlinear regressions', () => {
  assert.equal(evaluateExpectation('zero', [0, 0, 0], 'zero').pass, true);
  assert.equal(evaluateExpectation('zero', [0, 1, 0], 'constant').pass, false);
  assert.equal(evaluateExpectation('constant', [7, 7, 7], 'constant').pass, true);
  assert.equal(evaluateExpectation('constant', [7, 28, 112], 'superlinear').pass, false);
  assert.equal(evaluateExpectation('linear', [10, 20, 40], 'linear').pass, true);
  assert.equal(evaluateExpectation('linear', [10, 40, 160], 'superlinear').pass, false);
  assert.equal(evaluateExpectation('known-superlinear', [10, 40, 160], 'superlinear').pass, true);
});

test('evaluateExpectation warns (without failing) when constant turns linear', () => {
  const { pass, note } = evaluateExpectation('constant', [10, 20, 40], 'linear');
  assert.equal(pass, true);
  assert.match(note, /WARN/);
});

test('generators are deterministic and size-sensitive', () => {
  const dirA = mkdtempSync(path.join(os.tmpdir(), 'complexity-gen-a-'));
  const dirB = mkdtempSync(path.join(os.tmpdir(), 'complexity-gen-b-'));
  try {
    for (const generate of [
      generateModuleGraphProject,
      generateUnionProject,
      generateOverloadProject,
      generateInheritanceProject,
    ]) {
      const subA = path.join(dirA, generate.name);
      const subB = path.join(dirB, generate.name);
      const tsconfigA = generate(subA, 8);
      const tsconfigB = generate(subB, 8);
      assert.equal(readFileSync(tsconfigA, 'utf8'), readFileSync(tsconfigB, 'utf8'));
      const mainA = path.join(subA, 'src', generate === generateModuleGraphProject ? 'mod_0.ts' : 'index.ts');
      const mainB = path.join(subB, 'src', generate === generateModuleGraphProject ? 'mod_0.ts' : 'index.ts');
      assert.equal(readFileSync(mainA, 'utf8'), readFileSync(mainB, 'utf8'));
    }

    const smallDir = path.join(dirA, 'union-small');
    const largeDir = path.join(dirA, 'union-large');
    generateUnionProject(smallDir, 8);
    generateUnionProject(largeDir, 16);
    const small = readFileSync(path.join(smallDir, 'src', 'index.ts'), 'utf8');
    const large = readFileSync(path.join(largeDir, 'src', 'index.ts'), 'utf8');
    assert.ok(large.length > small.length);
    assert.match(large, /"k15"/);
    assert.doesNotMatch(small, /"k15"/);
  } finally {
    rmSync(dirA, { recursive: true, force: true });
    rmSync(dirB, { recursive: true, force: true });
  }
});

test('module graph generator writes the fixed dependency packages', () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'complexity-gen-deps-'));
  try {
    generateModuleGraphProject(dir, 8);
    for (let dep = 0; dep < MODULE_GRAPH_DEP_COUNT; dep += 1) {
      const declaration = readFileSync(
        path.join(dir, 'node_modules', `dep${dep}`, 'index.d.ts'),
        'utf8',
      );
      assert.match(declaration, new RegExp(`DepShape${dep}`));
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('projectSpecs reference counters with valid expectations', () => {
  const specs = projectSpecs();
  assert.ok(specs.length >= 5);
  for (const spec of specs) {
    assert.ok(spec.sizes.length >= 3, `${spec.name} needs at least 3 sizes`);
    assert.ok(spec.counterCases.length > 0);
    for (const counterCase of spec.counterCases) {
      assert.match(counterCase.counter, /^[a-z][a-z0-9_]*$/);
    }
  }
});

test('renderMarkdownReport includes rows, wall time, and determinism section', () => {
  const result: CounterCaseResult = {
    label: 'union member work',
    counter: 'union_type_clone_count',
    expected: 'linear',
    project: 'union-scaling',
    sizes: [64, 128],
    totals: [519, 1031],
    classification: 'linear',
    tailExponent: 0.99,
    fitExponent: 0.99,
    pass: true,
    note: 'ok',
  };
  const markdown = renderMarkdownReport(
    [result],
    new Map([['union-scaling', [31.5, 42.7]]]),
    [{ name: 'zod-shaped fixture', status: 'pass', note: 'sha256 abc… twice (exit 2)' }],
  );
  assert.match(markdown, /## union-scaling/);
  assert.match(markdown, /union_type_clone_count/);
  assert.match(markdown, /519 \| 1031/);
  assert.match(markdown, /~linear \(p=0\.99\)/);
  assert.match(markdown, /wall ms \(displayed, never gated\)/);
  assert.match(markdown, /## determinism/);
  assert.match(markdown, /PASS/);
});

test('renderMarkdownReport marks failures', () => {
  const result: CounterCaseResult = {
    label: 'dependency decl table clones',
    counter: 'dependency_declaration_table_clone_count',
    expected: 'zero',
    project: 'shared-checker-options',
    sizes: [64, 128],
    totals: [0, 3],
    classification: 'constant',
    tailExponent: 0.1,
    fitExponent: 0.1,
    pass: false,
    note: 'expected 0 at every size, got [0, 3]',
  };
  const markdown = renderMarkdownReport([result], new Map(), []);
  assert.match(markdown, /FAIL — expected 0 at every size/);
});

test('parseArgs handles flags and validates sizes', () => {
  const defaults = parseArgs([]);
  assert.equal(defaults.json, false);
  assert.equal(defaults.skipBuild, false);
  assert.equal(defaults.caseFilter, null);
  assert.equal(defaults.sizesOverride, null);
  assert.match(defaults.binary, /target[\\/]release[\\/]surge/);

  const parsed = parseArgs(['--', '--json', '--skipBuild', '--case', 'union', '--sizes', '8,16']);
  assert.equal(parsed.json, true);
  assert.equal(parsed.skipBuild, true);
  assert.equal(parsed.caseFilter, 'union');
  assert.deepEqual(parsed.sizesOverride, [8, 16]);

  assert.throws(() => parseArgs(['--sizes', '8']));
  assert.throws(() => parseArgs(['--sizes', '8,notanumber']));
  assert.throws(() => parseArgs(['--frobnicate']));
});

test('sha256 is stable and input-sensitive', () => {
  assert.equal(sha256('a'), sha256('a'));
  assert.notEqual(sha256('a'), sha256('b'));
  assert.match(sha256(''), /^[0-9a-f]{64}$/);
});
