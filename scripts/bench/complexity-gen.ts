/// Deterministic synthetic-project generators for the complexity regression
/// suite (complexity-regression.ts). Each generator writes a self-contained
/// zero-diagnostic project sized by `n` and returns the tsconfig path, so the
/// harness can compare instrumentation-counter totals across sizes.

import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

function writeTsconfig(dir: string): string {
  const tsconfigPath = path.join(dir, 'tsconfig.json');
  writeFileSync(
    tsconfigPath,
    `${JSON.stringify(
      {
        compilerOptions: {
          target: 'ES2022',
          module: 'CommonJS',
          strict: true,
          skipLibCheck: true,
          noEmit: true,
        },
        include: ['src/**/*.ts'],
      },
      null,
      2,
    )}\n`,
  );
  return tsconfigPath;
}

function writeSource(dir: string, relative: string, content: string): void {
  const filePath = path.join(dir, relative);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

export const MODULE_GRAPH_DEP_COUNT = 4;

/// `n` root modules; module i imports up to three earlier neighbors and one of
/// a fixed set of synthetic node_modules packages, so both root-source and
/// dependency-declaration table handling are exercised as the module count
/// grows while the dependency set stays constant.
export function generateModuleGraphProject(dir: string, moduleCount: number): string {
  for (let dep = 0; dep < MODULE_GRAPH_DEP_COUNT; dep += 1) {
    writeSource(
      dir,
      path.join('node_modules', `dep${dep}`, 'package.json'),
      `${JSON.stringify({ name: `dep${dep}`, version: '1.0.0', types: 'index.d.ts' }, null, 2)}\n`,
    );
    writeSource(
      dir,
      path.join('node_modules', `dep${dep}`, 'index.d.ts'),
      [
        `export interface DepShape${dep} {`,
        `  id: number;`,
        `  tag: "dep${dep}";`,
        `}`,
        ``,
        `export declare function load${dep}(): DepShape${dep};`,
        ``,
      ].join('\n'),
    );
  }

  for (let i = 0; i < moduleCount; i += 1) {
    const neighbors = [i - 1, i - 2, i - 4].filter((j) => j >= 0);
    const depIndex = i % MODULE_GRAPH_DEP_COUNT;
    const lines: string[] = [];
    for (const j of neighbors) {
      lines.push(`import { make${j} } from "./mod_${j}";`);
    }
    lines.push(`import { load${depIndex} } from "dep${depIndex}";`);
    lines.push('');
    lines.push(`export interface Shape${i} {`);
    lines.push(`  id: number;`);
    lines.push(`  name: string;`);
    lines.push(`  kind: "s${i}";`);
    lines.push(`}`);
    lines.push('');
    lines.push(`export function make${i}(id: number): Shape${i} {`);
    lines.push(`  return { id, name: "m${i}", kind: "s${i}" };`);
    lines.push(`}`);
    lines.push('');
    lines.push(`export const local${i}: Shape${i} = make${i}(${i});`);
    lines.push(`export const depId${i}: number = load${depIndex}().id;`);
    for (const j of neighbors) {
      lines.push(`export const from${i}_${j}: number = make${j}(${j}).id;`);
    }
    lines.push('');
    writeSource(dir, path.join('src', `mod_${i}.ts`), lines.join('\n'));
  }
  return writeTsconfig(dir);
}

function literalUnion(prefix: string, count: number, repeatEach: number): string {
  const members: string[] = [];
  for (let i = 0; i < count; i += 1) {
    for (let r = 0; r < repeatEach; r += 1) {
      members.push(`"${prefix}${i}"`);
    }
  }
  return members.join(' | ');
}

/// One file whose union shapes all scale with `n`. Literal-alias shapes (flat,
/// duplicated-member, nested halves, repeated canonical member lists) pin the
/// lazy/interned alias path, while a discriminated union routed through a
/// switch and `n` two-member conditional-expression unions drive actual union
/// member allocation, cloning, and handle-copy work.
export function generateUnionProject(dir: string, memberCount: number): string {
  const half = Math.max(1, Math.floor(memberCount / 2));
  const lines: string[] = [];

  lines.push(`type Flat = ${literalUnion('k', memberCount, 1)};`);
  lines.push(`const flat: Flat = "k0";`);
  lines.push('');
  lines.push(`type Dup = ${literalUnion('d', half, 2)};`);
  lines.push(`const dup: Dup = "d0";`);
  lines.push('');
  lines.push(`type NestedA = ${literalUnion('a', half, 1)};`);
  lines.push(`type NestedB = ${literalUnion('b', half, 1)};`);
  lines.push(`type Nested = NestedA | NestedB;`);
  lines.push(`const nested: Nested = "a0";`);
  lines.push('');
  for (let alias = 0; alias < 4; alias += 1) {
    lines.push(`type Canon${alias} = ${literalUnion('c', memberCount, 1)};`);
    lines.push(`const canon${alias}: Canon${alias} = "c0";`);
  }
  lines.push('');
  lines.push(`function pick(value: Flat): Flat {`);
  lines.push(`  return value;`);
  lines.push(`}`);
  lines.push(`const picked: Flat = pick("k1");`);
  lines.push('');

  for (let i = 0; i < memberCount; i += 1) {
    lines.push(`interface Variant${i} { kind: "v${i}"; value${i}: number; }`);
  }
  const variantNames: string[] = [];
  for (let i = 0; i < memberCount; i += 1) {
    variantNames.push(`Variant${i}`);
  }
  lines.push(`type AnyVariant = ${variantNames.join(' | ')};`);
  lines.push(`declare const anyVariant: AnyVariant;`);
  lines.push(`function route(input: AnyVariant): number {`);
  lines.push(`  switch (input.kind) {`);
  lines.push(`    case "v0": return input.value0;`);
  lines.push(`    case "v${half}": return input.value${half};`);
  lines.push(`    case "v${memberCount - 1}": return input.value${memberCount - 1};`);
  lines.push(`    default: return 0;`);
  lines.push(`  }`);
  lines.push(`}`);
  lines.push(`const routed: number = route(anyVariant);`);
  lines.push('');
  lines.push(`declare const flag: boolean;`);
  for (let i = 0; i < memberCount; i += 1) {
    lines.push(`const cond${i} = flag ? "a${i}" : ${i};`);
    lines.push(`const condUse${i}: string | number = cond${i};`);
  }
  lines.push('');
  lines.push(
    `export { flat, dup, nested, canon0, canon1, canon2, canon3, picked, routed };`,
  );
  lines.push('');
  writeSource(dir, path.join('src', 'index.ts'), lines.join('\n'));
  return writeTsconfig(dir);
}

export const OVERLOAD_PROBE_CALLS = 16;

/// Overload scaling in both directions: a growing n-overload `select` group
/// probed by a fixed number of calls (per-call cost over a large group), and n
/// calls against small fixed groups — an `on` group taking callbacks and a
/// `dup` group of duplicate identical signatures (per-call cost repeated many
/// times). Probe results and callback parameters stay annotated/unasserted:
/// which overload wins is checker semantics pinned elsewhere; this suite only
/// measures how much work selection does, so the file must stay diagnostic-free.
export function generateOverloadProject(dir: string, overloadCount: number): string {
  const lines: string[] = [];

  lines.push(`interface Api {`);
  for (let i = 0; i < overloadCount; i += 1) {
    lines.push(`  select(tag: "t${i}", value: number): "r${i}";`);
  }
  lines.push(`  on(kind: "str", callback: (input: string) => void): void;`);
  lines.push(`  on(kind: "num", callback: (input: number) => void): void;`);
  for (let d = 0; d < 4; d += 1) {
    lines.push(`  dup(flag: boolean): void;`);
  }
  lines.push(`}`);
  lines.push(`declare const api: Api;`);
  lines.push('');

  const probes = Math.min(OVERLOAD_PROBE_CALLS, overloadCount);
  for (let k = 0; k < probes; k += 1) {
    const index = Math.floor((k * overloadCount) / probes);
    lines.push(`const pick${k} = api.select("t${index}", ${k});`);
    lines.push(`void pick${k};`);
  }
  lines.push('');

  for (let i = 0; i < overloadCount; i += 1) {
    if (i % 2 === 0) {
      lines.push(`api.on("str", (input: string) => { void input; });`);
    } else {
      lines.push(`api.on("num", (input: number) => { void input; });`);
    }
    lines.push(`api.dup(${i % 2 === 0 ? 'true' : 'false'});`);
  }
  lines.push('');
  lines.push(`export {};`);
  lines.push('');
  writeSource(dir, path.join('src', 'index.ts'), lines.join('\n'));
  return writeTsconfig(dir);
}

/// Inheritance scaling: an n-deep single-extends chain (with a compatible
/// re-declared property every 8 levels) probed by first/middle/last property
/// reads, and an n-wide multi-extends interface whose bases share a repeated
/// `common` property.
export function generateInheritanceProject(dir: string, size: number): string {
  const lines: string[] = [];

  lines.push(`interface C0 {`);
  lines.push(`  p0: number;`);
  lines.push(`  shared: number;`);
  lines.push(`}`);
  for (let i = 1; i < size; i += 1) {
    lines.push(`interface C${i} extends C${i - 1} {`);
    lines.push(`  p${i}: number;`);
    if (i % 8 === 0) {
      lines.push(`  shared: number;`);
    }
    lines.push(`}`);
  }
  lines.push(`declare const deep: C${size - 1};`);
  lines.push(`const deepFirst: number = deep.p0;`);
  lines.push(`const deepMiddle: number = deep.p${Math.floor(size / 2)};`);
  lines.push(`const deepLast: number = deep.p${size - 1};`);
  lines.push(`const deepShared: number = deep.shared;`);
  lines.push('');

  const baseNames: string[] = [];
  for (let i = 0; i < size; i += 1) {
    lines.push(`interface B${i} {`);
    lines.push(`  common: number;`);
    lines.push(`  w${i}: number;`);
    lines.push(`}`);
    baseNames.push(`B${i}`);
  }
  lines.push(`interface WideAll extends ${baseNames.join(', ')} {`);
  lines.push(`  own: string;`);
  lines.push(`}`);
  lines.push(`declare const wide: WideAll;`);
  lines.push(`const wideCommon: number = wide.common;`);
  lines.push(`const wideFirst: number = wide.w0;`);
  lines.push(`const wideLast: number = wide.w${size - 1};`);
  lines.push(`const wideOwn: string = wide.own;`);
  lines.push('');
  lines.push(
    `export { deepFirst, deepMiddle, deepLast, deepShared, wideCommon, wideFirst, wideLast, wideOwn };`,
  );
  lines.push('');
  writeSource(dir, path.join('src', 'index.ts'), lines.join('\n'));
  return writeTsconfig(dir);
}
