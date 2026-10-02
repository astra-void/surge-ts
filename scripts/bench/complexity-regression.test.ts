import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { expect, test } from 'vitest';

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
  expect(counters.get('files_total')).toBe(12);
  expect(counters.get('type_declaration_table_clone_count')).toBe(2);
  expect(counters.get('dependency_declaration_table_clone_count')).toBe(0);
  expect(counters.get('union_type_clone_count')).toBe(519);
  expect(counters.get('declaration_lookup_layer_count_avg')).toBe(1.52);
  expect(counters.get('overload_array_alloc_count')).toBe(35);
  expect(counters.has('canonicalize_calls')).toBe(false);
  expect(counters.has('parsing')).toBe(false);
  expect(counters.has('total')).toBe(false);
});

test('parseTimingsCounters returns empty map when no counters block exists', () => {
  expect(parseTimingsCounters('Timings:\n  parsing: 1.0ms\n').size).toBe(0);
});

test('classifyGrowth: all-zero series', () => {
  const { classification, tailExponent } = classifyGrowth([64, 128, 256], [0, 0, 0]);
  expect(classification).toBe('zero');
  expect(tailExponent).toBe(null);
});

test('classifyGrowth: constant series', () => {
  const { classification } = classifyGrowth([64, 128, 256], [7, 7, 7]);
  expect(classification).toBe('constant');
});

test('classifyGrowth: linear series, including fixed offset', () => {
  expect(classifyGrowth([64, 128, 256], [100, 200, 400]).classification).toBe('linear');
  // 50 fixed + n: the tail exponent converges to 1 despite the offset.
  expect(classifyGrowth([64, 128, 256, 512], [114, 178, 306, 562]).classification).toBe('linear');
});

test('classifyGrowth: quadratic series is superlinear', () => {
  const sizes = [64, 128, 256];
  const totals = sizes.map((n) => n * n);
  const { classification, tailExponent } = classifyGrowth(sizes, totals);
  expect(classification).toBe('superlinear');
  expect(tailExponent! > 1.9).toBeTruthy();
});

test('classifyGrowth rejects mismatched or too-short input', () => {
  expect(() => classifyGrowth([64], [1])).toThrow();
  expect(() => classifyGrowth([64, 128], [1])).toThrow();
});

test('evaluateExpectation gates zero and superlinear regressions', () => {
  expect(evaluateExpectation('zero', [0, 0, 0], 'zero').pass).toBe(true);
  expect(evaluateExpectation('zero', [0, 1, 0], 'constant').pass).toBe(false);
  expect(evaluateExpectation('constant', [7, 7, 7], 'constant').pass).toBe(true);
  expect(evaluateExpectation('constant', [7, 28, 112], 'superlinear').pass).toBe(false);
  expect(evaluateExpectation('linear', [10, 20, 40], 'linear').pass).toBe(true);
  expect(evaluateExpectation('linear', [10, 40, 160], 'superlinear').pass).toBe(false);
  expect(evaluateExpectation('known-superlinear', [10, 40, 160], 'superlinear').pass).toBe(true);
});

test('evaluateExpectation warns (without failing) when constant turns linear', () => {
  const { pass, note } = evaluateExpectation('constant', [10, 20, 40], 'linear');
  expect(pass).toBe(true);
  expect(note).toMatch(/WARN/);
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
      expect(readFileSync(tsconfigA, 'utf8')).toBe(readFileSync(tsconfigB, 'utf8'));
      const mainA = path.join(subA, 'src', generate === generateModuleGraphProject ? 'mod_0.ts' : 'index.ts');
      const mainB = path.join(subB, 'src', generate === generateModuleGraphProject ? 'mod_0.ts' : 'index.ts');
      expect(readFileSync(mainA, 'utf8')).toBe(readFileSync(mainB, 'utf8'));
    }

    const smallDir = path.join(dirA, 'union-small');
    const largeDir = path.join(dirA, 'union-large');
    generateUnionProject(smallDir, 8);
    generateUnionProject(largeDir, 16);
    const small = readFileSync(path.join(smallDir, 'src', 'index.ts'), 'utf8');
    const large = readFileSync(path.join(largeDir, 'src', 'index.ts'), 'utf8');
    expect(large.length > small.length).toBeTruthy();
    expect(large).toMatch(/"k15"/);
    expect(small).not.toMatch(/"k15"/);
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
      expect(declaration).toMatch(new RegExp(`DepShape${dep}`));
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('projectSpecs reference counters with valid expectations', () => {
  const specs = projectSpecs();
  expect(specs.length >= 5).toBeTruthy();
  for (const spec of specs) {
    expect(spec.sizes.length >= 3, `${spec.name} needs at least 3 sizes`).toBeTruthy();
    expect(spec.counterCases.length > 0).toBeTruthy();
    for (const counterCase of spec.counterCases) {
      expect(counterCase.counter).toMatch(/^[a-z][a-z0-9_]*$/);
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
  expect(markdown).toMatch(/## union-scaling/);
  expect(markdown).toMatch(/union_type_clone_count/);
  expect(markdown).toMatch(/519 \| 1031/);
  expect(markdown).toMatch(/~linear \(p=0\.99\)/);
  expect(markdown).toMatch(/wall ms \(displayed, never gated\)/);
  expect(markdown).toMatch(/## determinism/);
  expect(markdown).toMatch(/PASS/);
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
  expect(markdown).toMatch(/FAIL — expected 0 at every size/);
});

test('parseArgs handles flags and validates sizes', () => {
  const defaults = parseArgs([]);
  expect(defaults.json).toBe(false);
  expect(defaults.skipBuild).toBe(false);
  expect(defaults.caseFilter).toBe(null);
  expect(defaults.sizesOverride).toBe(null);
  expect(defaults.binary).toMatch(/target[\\/]release[\\/]surge/);

  const parsed = parseArgs(['--', '--json', '--skipBuild', '--case', 'union', '--sizes', '8,16']);
  expect(parsed.json).toBe(true);
  expect(parsed.skipBuild).toBe(true);
  expect(parsed.caseFilter).toBe('union');
  expect(parsed.sizesOverride).toStrictEqual([8, 16]);

  expect(() => parseArgs(['--sizes', '8'])).toThrow();
  expect(() => parseArgs(['--sizes', '8,notanumber'])).toThrow();
  expect(() => parseArgs(['--frobnicate'])).toThrow();
});

test('sha256 is stable and input-sensitive', () => {
  expect(sha256('a')).toBe(sha256('a'));
  expect(sha256('a')).not.toBe(sha256('b'));
  expect(sha256('')).toMatch(/^[0-9a-f]{64}$/);
});
