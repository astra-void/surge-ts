// Held-out upstream tier for compare-checkers.ts: TypeScript's own test cases,
// minus every case either checker already tests against. bolt-ts's suite is
// ~3,500 of these cases and surge-ts pins a curated upstream set, so running
// the full pool would score each tool partly on its own regression tests.

import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';

export type UpstreamCaseFile = { name: string; content: string };
export type ParsedUpstreamCase = { directives: Array<[string, string]>; files: UpstreamCaseFile[] };

// `// @name: value`, the pattern TypeScript's harness reads metadata with.
const directivePattern = /^\/\/\s*@(\w+)\s*:\s*([^\r\n]*)/;

// Mirrors the harness's `makeUnitsFromTest`: directive lines are metadata, not
// source, and each `@filename` starts a new unit.
export function parseUpstreamCase(text: string, defaultFileName: string): ParsedUpstreamCase {
  const directives: Array<[string, string]> = [];
  const files: UpstreamCaseFile[] = [];
  let currentName: string | undefined;
  let currentContent: string | undefined;
  for (const line of text.split(/\r?\n/)) {
    const match = directivePattern.exec(line);
    if (!match) {
      currentContent = currentContent === undefined ? line : `${currentContent}\n${line}`;
      continue;
    }
    const value = match[2].trim();
    if (match[1].toLowerCase() !== 'filename') {
      directives.push([match[1], value]);
      continue;
    }
    if (currentName !== undefined) {
      files.push({ name: currentName, content: currentContent ?? '' });
    }
    currentName = value;
    currentContent = undefined;
  }
  files.push({ name: currentName ?? defaultFileName, content: currentContent ?? '' });
  return { directives, files };
}

type OptionDeclaration = { name: string; type: unknown };

let optionTable: Map<string, OptionDeclaration> | null = null;

function loadOptionTable(typescriptPath: string): Map<string, OptionDeclaration> {
  if (!optionTable) {
    const ts = createRequire(import.meta.url)(typescriptPath) as { optionDeclarations: OptionDeclaration[] };
    optionTable = new Map(ts.optionDeclarations.map((option) => [option.name.toLowerCase(), option]));
  }
  return optionTable;
}

// Directives that need harness machinery a plain tsconfig cannot express.
const harnessOnlyDirectives = new Set(['link', 'symlink', 'currentdirectory', 'libfiles']);
// Every tool runs `noEmit`; these only configure output and several are
// rejected alongside `noEmit`, which would turn a checker test into a config error.
const emitOnlyOptions = new Set([
  'emitdeclarationonly', 'outfile', 'out', 'outdir', 'declarationdir', 'sourcemap', 'inlinesourcemap', 'maproot',
  'sourceroot', 'declarationmap', 'inlinesources', 'emitbom', 'newline', 'removecomments', 'noemit',
  'noemithelpers', 'noemitonerror', 'stripinternal', 'composite', 'incremental', 'tsbuildinfofile',
]);
const sourceExtensions = /\.(ts|tsx|mts|cts)$/;
const scriptExtensions = /\.(js|jsx|mjs|cjs)$/;

export type CaseConfig = { compilerOptions: Record<string, unknown>; files: string[] };

export function caseConfig(parsed: ParsedUpstreamCase, typescriptPath: string): CaseConfig | { skip: string } {
  const table = loadOptionTable(typescriptPath);
  const compilerOptions: Record<string, unknown> = {};
  for (const [rawName, rawValue] of parsed.directives) {
    const key = rawName.toLowerCase();
    if (harnessOnlyDirectives.has(key)) return { skip: `harness directive @${rawName}` };
    const option = table.get(key);
    if (!option || emitOnlyOptions.has(key)) continue;
    if (option.type === 'list') {
      compilerOptions[option.name] = rawValue.split(',').map((item) => item.trim()).filter(Boolean);
      continue;
    }
    // A comma list is a variation matrix; the first entry is one real config.
    const value = rawValue.split(',')[0].trim();
    if (!value || value.includes('*') || value.startsWith('-')) continue;
    if (option.type === 'boolean') compilerOptions[option.name] = value.toLowerCase() === 'true';
    else if (option.type === 'number') compilerOptions[option.name] = Number(value);
    else compilerOptions[option.name] = value;
  }
  compilerOptions.noEmit = true;

  const allowJs = compilerOptions.allowJs === true || compilerOptions.checkJs === true;
  const files: string[] = [];
  for (const file of parsed.files) {
    const name = relativeCaseFileName(file.name);
    if (name === null) return { skip: `file outside the case: ${file.name}` };
    if (path.basename(name).toLowerCase() === 'tsconfig.json') return { skip: 'case supplies its own tsconfig.json' };
    if (name.split('/').includes('node_modules')) continue;
    if (sourceExtensions.test(name) || (allowJs && scriptExtensions.test(name))) files.push(name);
  }
  if (files.length === 0) return { skip: 'no root source files' };
  return { compilerOptions, files };
}

function relativeCaseFileName(name: string): string | null {
  const normalized = path.posix.normalize(name.replace(/\\/g, '/').replace(/^[a-zA-Z]:/, '').replace(/^\/+/, ''));
  return normalized.startsWith('..') ? null : normalized;
}

// Whitespace- and directive-insensitive, so a copy that was re-indented or had
// its `@` metadata edited still counts as the same test.
export function caseFingerprint(text: string): string {
  const body = text
    .split(/\r?\n/)
    .filter((line) => !directivePattern.test(line))
    .join('')
    .replace(/\s+/g, '');
  return createHash('sha1').update(body).digest('hex');
}

export type UpstreamCaseSource = { name: string; path: string };

// Later roots never shadow an earlier one: typescript-go's own cases fill in
// names the TypeScript submodule does not have.
export function listUpstreamCases(roots: string[]): UpstreamCaseSource[] {
  const byName = new Map<string, UpstreamCaseSource>();
  const walk = (dir: string) => {
    for (const entry of readdirSync(dir)) {
      const full = path.join(dir, entry);
      if (statSync(full).isDirectory()) {
        walk(full);
      } else if (/\.(ts|tsx)$/.test(entry)) {
        const name = entry.replace(/\.(d\.)?(ts|tsx)$/, '');
        if (!byName.has(name)) byName.set(name, { name, path: full });
      }
    }
  };
  for (const root of roots) {
    if (existsSync(root)) walk(root);
  }
  return [...byName.values()].sort((a, b) => a.name.localeCompare(b.name));
}

export type OwnTestSuite = { label: string; names: Set<string>; fingerprints: Set<string> };

// bolt-ts keeps one directory per case under tests/compiler, named after the
// upstream case, sometimes with an option suffix (`foo_strict`).
export function boltTestSuite(boltRepo: string, caseNames: Set<string>): OwnTestSuite {
  const dir = path.join(boltRepo, 'tests', 'compiler');
  if (!existsSync(dir)) {
    throw new Error(`bolt-ts test suite not found at ${dir}; the held-out tier cannot exclude its cases`);
  }
  const names = new Set<string>();
  const fingerprints = new Set<string>();
  for (const entry of readdirSync(dir)) {
    let name = entry;
    while (!caseNames.has(name) && name.includes('_')) {
      name = name.slice(0, name.lastIndexOf('_'));
    }
    names.add(caseNames.has(name) ? name : entry);
    const source = path.join(dir, entry, 'index.ts');
    if (existsSync(source)) fingerprints.add(caseFingerprint(readFileSync(source, 'utf8')));
  }
  return { label: 'bolt-ts', names, fingerprints };
}

export function surgeTestSuite(workspaceRoot: string): OwnTestSuite {
  const manifest = readFileSync(path.join(workspaceRoot, 'tests', 'upstream', 'typescript-go', 'manifest.toml'), 'utf8');
  const names = new Set<string>();
  const fingerprints = new Set<string>();
  for (const match of manifest.matchAll(/^upstream_path = "([^"]+)"$/gm)) {
    names.add(path.basename(match[1]).replace(/\.(ts|tsx)$/, ''));
  }
  for (const match of manifest.matchAll(/^local_path = "([^"]+)"$/gm)) {
    const local = path.join(workspaceRoot, match[1]);
    if (existsSync(local)) fingerprints.add(caseFingerprint(readFileSync(local, 'utf8')));
  }
  return { label: 'surge-ts', names, fingerprints };
}

export type HeldOutSelection = {
  cases: UpstreamCaseSource[];
  pool: number;
  excluded: Record<string, number>;
  sampled: number;
};

export function selectHeldOutCases(
  pool: UpstreamCaseSource[],
  suites: OwnTestSuite[],
  limit: number,
): HeldOutSelection {
  const excluded: Record<string, number> = Object.fromEntries(suites.map((suite) => [suite.label, 0]));
  const kept: UpstreamCaseSource[] = [];
  for (const source of pool) {
    const fingerprint = caseFingerprint(readFileSync(source.path, 'utf8'));
    const owner = suites.find((suite) => suite.names.has(source.name) || suite.fingerprints.has(fingerprint));
    if (owner) excluded[owner.label]++;
    else kept.push(source);
  }
  // Hash order, not name order, so a limited run is a spread sample rather
  // than the alphabetical head of the suite.
  const rank = (name: string) => createHash('sha1').update(name).digest('hex');
  const ordered = kept.sort((a, b) => rank(a.name).localeCompare(rank(b.name)));
  const cases = limit > 0 ? ordered.slice(0, limit) : ordered;
  return { cases, pool: pool.length, excluded, sampled: cases.length };
}

export function materializeCase(
  source: UpstreamCaseSource,
  outDir: string,
  typescriptPath: string,
): { tsconfig: string } | { skip: string } {
  const text = readFileSync(source.path, 'utf8');
  const parsed = parseUpstreamCase(text, path.basename(source.path));
  const config = caseConfig(parsed, typescriptPath);
  if ('skip' in config) return config;
  const dir = path.join(outDir, source.name);
  rmSync(dir, { recursive: true, force: true });
  for (const file of parsed.files) {
    const target = path.join(dir, relativeCaseFileName(file.name) ?? file.name);
    mkdirSync(path.dirname(target), { recursive: true });
    writeFileSync(target, file.content);
  }
  const tsconfig = path.join(dir, 'tsconfig.json');
  writeFileSync(tsconfig, `${JSON.stringify(config, null, 2)}\n`);
  return { tsconfig };
}
