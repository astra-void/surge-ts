#!/usr/bin/env tsx

import { spawnSync } from 'node:child_process';
import { existsSync, statSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

type Source = 'typescript' | 'typescript-rust';

type NormalizedDiagnostic = {
  source: Source;
  code: string;
  fileName: string;
  line?: number;
  column?: number;
  message?: string;
};

type CountBucket = {
  key: string;
  typescript: number;
  typescriptRust: number;
};

type CountEntry = {
  key: string;
  count: number;
};

type ComparisonResult = {
  project: string;
  typescript: {
    total: number;
    byCode: CountEntry[];
    byFileCode: CountEntry[];
    byFileCodeLine: CountEntry[];
  };
  typescriptRust: {
    total: number;
    byCode: CountEntry[];
    byFileCode: CountEntry[];
    byFileCodeLine: CountEntry[];
  };
  matches: {
    byCode: CountBucket[];
    onlyTypeScript: CountBucket[];
    onlyTypeScriptRust: CountBucket[];
    byFileCode: CountBucket[];
    onlyTypeScriptFileCode: CountBucket[];
    onlyTypeScriptRustFileCode: CountBucket[];
    byFileCodeLine: CountBucket[];
    onlyTypeScriptFileCodeLine: CountBucket[];
    onlyTypeScriptRustFileCodeLine: CountBucket[];
  };
  summary: {
    byCodeMatch: boolean;
    byFileCodeMatch: boolean;
    byFileCodeLineMatch: boolean | null;
  };
};

type ParsedArgs = {
  projectInput?: string;
  json: boolean;
  failOnMismatch: boolean;
  maxDiagnostics?: number;
};

const scriptPath = fileURLToPath(import.meta.url);
const scriptDir = path.dirname(scriptPath);
const workspaceRoot = path.resolve(scriptDir, '../..');
const npmCache = process.env.npm_config_cache ?? path.join(os.tmpdir(), 'codex-npm-cache');

const fixturePresets: Record<string, string> = {
  'generics-basic': path.join(workspaceRoot, 'tests/compat-projects/generics-basic/tsconfig.json'),
  'package-imports': path.join(workspaceRoot, 'tests/compat-projects/package-imports/tsconfig.json'),
  'module-forms': path.join(workspaceRoot, 'tests/compat-projects/module-forms/tsconfig.json'),
  'relative-deep': path.join(workspaceRoot, 'tests/compat-projects/relative-deep/tsconfig.json'),
  'private-types': path.join(workspaceRoot, 'tests/compat-projects/private-types/tsconfig.json'),
};

function main() {
  const args = parseArgs(process.argv.slice(2));
  const tsconfigPath = resolveProjectInput(args.projectInput);
  const projectDir = path.dirname(tsconfigPath);
  const projectDisplay = displayProjectPath(tsconfigPath);
  const relativeTsconfig = normalizePosixPath(path.relative(projectDir, tsconfigPath) || path.basename(tsconfigPath));

  const tsc = runTsc(projectDir, relativeTsconfig);
  const rust = runTypeScriptRust(projectDir, relativeTsconfig, args.maxDiagnostics);

  const tscDiagnostics = limitDiagnostics(
    parseTypeScriptDiagnostics(tsc.output, projectDir),
    args.maxDiagnostics,
  );
  const rustDiagnostics = limitDiagnostics(
    parseTypeScriptRustDiagnostics(rust.output, projectDir),
    args.maxDiagnostics,
  );

  const comparison = compareDiagnostics(projectDisplay, tscDiagnostics, rustDiagnostics);

  if (args.json) {
    process.stdout.write(`${JSON.stringify(comparison, null, 2)}\n`);
  } else {
    process.stdout.write(renderComparisonText(comparison));
  }

  const hasMismatch = !comparison.summary.byCodeMatch || !comparison.summary.byFileCodeMatch;
  if (args.failOnMismatch && hasMismatch) {
    process.exitCode = 1;
  }
}

function parseArgs(argv: string[]): ParsedArgs {
  const parsed: ParsedArgs = {
    json: false,
    failOnMismatch: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === '--') {
      continue;
    } else if (arg === '--help' || arg === '-h') {
      printHelpAndExit();
    } else if (arg === '--project' || arg === '--fixture') {
      const value = argv[++index];
      if (!value) {
        throw new Error(`${arg} requires a value`);
      }
      parsed.projectInput = value;
    } else if (arg === '--json') {
      parsed.json = true;
    } else if (arg === '--failOnMismatch' || arg === '--strictCodes') {
      parsed.failOnMismatch = true;
    } else if (arg === '--maxDiagnostics') {
      const value = argv[++index];
      if (!value) {
        throw new Error('--maxDiagnostics requires a value');
      }
      const parsedValue = Number(value);
      if (!Number.isInteger(parsedValue) || parsedValue <= 0) {
        throw new Error('--maxDiagnostics must be greater than 0');
      }
      parsed.maxDiagnostics = parsedValue;
    } else if (arg.startsWith('--')) {
      throw new Error(`unknown argument: ${arg}`);
    } else if (!parsed.projectInput) {
      parsed.projectInput = arg;
    } else {
      throw new Error(`unexpected extra argument: ${arg}`);
    }
  }

  return parsed;
}

function printHelpAndExit(): never {
  process.stdout.write(
    [
      'Usage:',
      '  pnpm run oracle:compare -- --project <tsconfig.json|preset>',
      '  pnpm run oracle:compare -- <tsconfig.json|preset>',
      '',
      'Options:',
      '  --project <path|preset>   Compare a tsconfig file or known fixture preset.',
      '  --fixture <preset>        Alias for --project when passing a preset name.',
      '  --maxDiagnostics <n>      Limit diagnostics on both sides before comparing.',
      '  --json                    Emit machine-readable comparison output.',
      '  --failOnMismatch          Exit with code 1 when code/file mismatches exist.',
      '  --strictCodes             Alias for --failOnMismatch.',
      '',
      'Known presets:',
      `  ${Object.keys(fixturePresets).join(', ')}`,
      '',
    ].join('\n'),
  );
  process.exit(0);
}

function resolveProjectInput(projectInput?: string): string {
  if (!projectInput) {
    throw new Error(
      'missing project input. Pass --project <tsconfig.json|preset> or a positional tsconfig path.',
    );
  }

  const candidate = projectInput;
  const resolved = path.resolve(workspaceRoot, candidate);

  if (existsSync(resolved)) {
    const stats = statSync(resolved);
    return stats.isDirectory() ? path.join(resolved, 'tsconfig.json') : resolved;
  }

  const preset = fixturePresets[candidate];
  if (preset) {
    return preset;
  }

  throw new Error(
    `could not resolve project input "${candidate}". Pass a tsconfig.json path or one of: ${Object.keys(
      fixturePresets,
    ).join(', ')}`,
  );
}

function displayProjectPath(tsconfigPath: string): string {
  const relative = path.relative(workspaceRoot, tsconfigPath);
  return relative.startsWith('..') ? normalizePosixPath(tsconfigPath) : normalizePosixPath(relative);
}

function runTsc(projectDir: string, relativeTsconfig: string): { output: string } {
  const result = spawnSync(
    'pnpm',
    ['exec', 'tsc', '--noEmit', '--pretty', 'false', '--project', relativeTsconfig],
    {
      cwd: projectDir,
      encoding: 'utf8',
      env: {
        ...process.env,
        npm_config_cache: npmCache,
      },
    },
  );

  if (result.error) {
    throw result.error;
  }

  return { output: `${result.stdout ?? ''}${result.stderr ?? ''}` };
}

function runTypeScriptRust(
  projectDir: string,
  relativeTsconfig: string,
  maxDiagnostics?: number,
): { output: string } {
  const args = [
    'run',
    '-q',
    '--manifest-path',
    path.join(workspaceRoot, 'Cargo.toml'),
    '-p',
    'typescript-rust-cli',
    '--',
    '--project',
    relativeTsconfig,
    '--format',
    'json',
  ];

  if (maxDiagnostics !== undefined) {
    args.push('--maxDiagnostics', String(maxDiagnostics));
  }

  const result = spawnSync('cargo', args, {
    cwd: projectDir,
    encoding: 'utf8',
  });

  if (result.error) {
    throw result.error;
  }

  return { output: result.stdout ?? '' };
}

function parseTypeScriptDiagnostics(output: string, projectDir: string): NormalizedDiagnostic[] {
  const diagnostics: NormalizedDiagnostic[] = [];
  const lines = output.split(/\r?\n/);

  for (const line of lines) {
    const matched = line.match(/^(.*)\((\d+),(\d+)\): error (TS\d+): (.*)$/);
    if (matched) {
      diagnostics.push({
        source: 'typescript',
        fileName: normalizeDiagnosticFileName(projectDir, matched[1]),
        line: Number(matched[2]),
        column: Number(matched[3]),
        code: matched[4],
        message: matched[5],
      });
      continue;
    }

    const globalError = line.match(/^error (TS\d+): (.*)$/);
    if (globalError) {
      diagnostics.push({
        source: 'typescript',
        fileName: '',
        code: globalError[1],
        message: globalError[2],
      });
    }
  }

  return diagnostics;
}

function parseTypeScriptRustDiagnostics(output: string, projectDir: string): NormalizedDiagnostic[] {
  const parsed = JSON.parse(output) as { diagnostics?: unknown };
  const diagnostics = Array.isArray(parsed.diagnostics) ? parsed.diagnostics : [];

  return diagnostics.map((diagnostic) => {
    const entry = diagnostic as {
      code?: unknown;
      fileName?: unknown;
      line?: unknown;
      column?: unknown;
      message?: unknown;
    };

    return {
      source: 'typescript-rust',
      code: String(entry.code ?? ''),
      fileName: normalizeDiagnosticFileName(projectDir, String(entry.fileName ?? '')),
      line: typeof entry.line === 'number' ? entry.line : undefined,
      column: typeof entry.column === 'number' ? entry.column : undefined,
      message: typeof entry.message === 'string' ? entry.message : undefined,
    };
  });
}

function normalizeDiagnosticFileName(projectDir: string, fileName: string): string {
  if (!fileName) {
    return '';
  }

  const absolute = path.isAbsolute(fileName) ? fileName : path.resolve(projectDir, fileName);
  const relative = path.relative(projectDir, absolute);

  if (relative && !relative.startsWith('..') && !path.isAbsolute(relative)) {
    return normalizePosixPath(relative);
  }

  return normalizePosixPath(fileName);
}

function normalizePosixPath(value: string): string {
  return value.split(path.sep).join('/');
}

function limitDiagnostics(
  diagnostics: NormalizedDiagnostic[],
  maxDiagnostics?: number,
): NormalizedDiagnostic[] {
  if (maxDiagnostics === undefined) {
    return diagnostics;
  }

  return diagnostics.slice(0, maxDiagnostics);
}

function compareDiagnostics(
  project: string,
  typescript: NormalizedDiagnostic[],
  typescriptRust: NormalizedDiagnostic[],
): ComparisonResult {
  const byCode = compareBuckets(typescript, typescriptRust, keyByCode);
  const byFileCode = compareBuckets(typescript, typescriptRust, keyByFileCode);
  const byFileCodeLine = compareBuckets(
    typescript.filter(hasLineInfo),
    typescriptRust.filter(hasLineInfo),
    keyByFileCodeLine,
  );

  return {
    project,
    typescript: {
      total: typescript.length,
      byCode: countEntriesFromCounts(countDiagnostics(typescript, keyByCode)),
      byFileCode: countEntriesFromCounts(countDiagnostics(typescript, keyByFileCode)),
      byFileCodeLine: countEntriesFromCounts(
        countDiagnostics(typescript.filter(hasLineInfo), keyByFileCodeLine),
      ),
    },
    typescriptRust: {
      total: typescriptRust.length,
      byCode: countEntriesFromCounts(countDiagnostics(typescriptRust, keyByCode)),
      byFileCode: countEntriesFromCounts(countDiagnostics(typescriptRust, keyByFileCode)),
      byFileCodeLine: countEntriesFromCounts(
        countDiagnostics(typescriptRust.filter(hasLineInfo), keyByFileCodeLine),
      ),
    },
    matches: {
      byCode: byCode.matches,
      onlyTypeScript: byCode.onlyTypeScript,
      onlyTypeScriptRust: byCode.onlyTypeScriptRust,
      byFileCode: byFileCode.matches,
      onlyTypeScriptFileCode: byFileCode.onlyTypeScript,
      onlyTypeScriptRustFileCode: byFileCode.onlyTypeScriptRust,
      byFileCodeLine: byFileCodeLine.matches,
      onlyTypeScriptFileCodeLine: byFileCodeLine.onlyTypeScript,
      onlyTypeScriptRustFileCodeLine: byFileCodeLine.onlyTypeScriptRust,
    },
    summary: {
      byCodeMatch: byCode.onlyTypeScript.length === 0 && byCode.onlyTypeScriptRust.length === 0,
      byFileCodeMatch:
        byFileCode.onlyTypeScript.length === 0 && byFileCode.onlyTypeScriptRust.length === 0,
      byFileCodeLineMatch:
        byFileCodeLine.matches.length > 0 ||
        byFileCodeLine.onlyTypeScript.length > 0 ||
        byFileCodeLine.onlyTypeScriptRust.length > 0
          ? byFileCodeLine.onlyTypeScript.length === 0 &&
            byFileCodeLine.onlyTypeScriptRust.length === 0
          : null,
    },
  };
}

function compareBuckets(
  left: NormalizedDiagnostic[],
  right: NormalizedDiagnostic[],
  keyFn: (diagnostic: NormalizedDiagnostic) => string,
): {
  matches: CountBucket[];
  onlyTypeScript: CountBucket[];
  onlyTypeScriptRust: CountBucket[];
} {
  const leftCounts = countDiagnostics(left, keyFn);
  const rightCounts = countDiagnostics(right, keyFn);
  const keys = new Set([...leftCounts.keys(), ...rightCounts.keys()]);
  const sortedKeys = [...keys].sort((leftKey, rightKey) => leftKey.localeCompare(rightKey));
  const matches: CountBucket[] = [];
  const onlyTypeScript: CountBucket[] = [];
  const onlyTypeScriptRust: CountBucket[] = [];

  for (const key of sortedKeys) {
    const leftCount = leftCounts.get(key) ?? 0;
    const rightCount = rightCounts.get(key) ?? 0;
    if (leftCount === rightCount) {
      if (leftCount > 0) {
        matches.push({ key, typescript: leftCount, typescriptRust: rightCount });
      }
      continue;
    }

    if (leftCount > 0 && rightCount === 0) {
      onlyTypeScript.push({ key, typescript: leftCount, typescriptRust: 0 });
      continue;
    }

    if (rightCount > 0 && leftCount === 0) {
      onlyTypeScriptRust.push({ key, typescript: 0, typescriptRust: rightCount });
      continue;
    }

    if (leftCount > rightCount) {
      onlyTypeScript.push({ key, typescript: leftCount, typescriptRust: rightCount });
    } else {
      onlyTypeScriptRust.push({ key, typescript: leftCount, typescriptRust: rightCount });
    }
  }

  return { matches, onlyTypeScript, onlyTypeScriptRust };
}

function countDiagnostics(
  diagnostics: NormalizedDiagnostic[],
  keyFn: (diagnostic: NormalizedDiagnostic) => string,
): Map<string, number> {
  const counts = new Map<string, number>();

  for (const diagnostic of diagnostics) {
    const key = keyFn(diagnostic);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }

  return counts;
}

function countEntriesFromCounts(counts: Map<string, number>): CountEntry[] {
  return [...counts.entries()]
    .map(([key, count]) => ({ key, count }))
    .sort((left, right) => left.key.localeCompare(right.key));
}

function keyByCode(diagnostic: NormalizedDiagnostic): string {
  return diagnostic.code;
}

function keyByFileCode(diagnostic: NormalizedDiagnostic): string {
  return `${diagnostic.fileName} :: ${diagnostic.code}`;
}

function keyByFileCodeLine(diagnostic: NormalizedDiagnostic): string {
  const line = diagnostic.line ?? 0;
  return `${diagnostic.fileName} :: ${diagnostic.code} :: line=${line}`;
}

function hasLineInfo(diagnostic: NormalizedDiagnostic): boolean {
  return typeof diagnostic.line === 'number' && typeof diagnostic.column === 'number';
}

function renderComparisonText(comparison: ComparisonResult): string {
  const lines: string[] = [];
  lines.push('TypeScript oracle comparison');
  lines.push(`Project: ${comparison.project}`);
  lines.push('');
  lines.push('Files:');
  lines.push(`TypeScript diagnostics: ${comparison.typescript.total}`);
  lines.push(`typescript-rust diagnostics: ${comparison.typescriptRust.total}`);
  lines.push('');
  lines.push('By code:');
  appendBucketSection(lines, comparison.matches.byCode, comparison.matches.onlyTypeScript, comparison.matches.onlyTypeScriptRust);
  lines.push('');
  lines.push('By file/code:');
  appendBucketSection(
    lines,
    comparison.matches.byFileCode,
    comparison.matches.onlyTypeScriptFileCode,
    comparison.matches.onlyTypeScriptRustFileCode,
  );
  lines.push('');
  lines.push('By file/code/line:');
  if (comparison.summary.byFileCodeLineMatch === null) {
    lines.push('  (no line information on both sides)');
  } else {
    appendBucketSection(
      lines,
      comparison.matches.byFileCodeLine,
      comparison.matches.onlyTypeScriptFileCodeLine,
      comparison.matches.onlyTypeScriptRustFileCodeLine,
    );
  }
  lines.push('');
  lines.push('Summary:');
  lines.push(`Code-count match: ${comparison.summary.byCodeMatch ? 'yes' : 'no'}`);
  lines.push(`File/code match: ${comparison.summary.byFileCodeMatch ? 'yes' : 'no'}`);
  lines.push(
    `File/code/line match: ${
      comparison.summary.byFileCodeLineMatch === null
        ? 'n/a'
        : comparison.summary.byFileCodeLineMatch
          ? 'yes'
          : 'no'
    }`,
  );
  return `${lines.join('\n')}\n`;
}

function appendBucketSection(
  lines: string[],
  matches: CountBucket[],
  onlyTypeScript: CountBucket[],
  onlyTypeScriptRust: CountBucket[],
) {
  if (matches.length === 0 && onlyTypeScript.length === 0 && onlyTypeScriptRust.length === 0) {
    lines.push('  (none)');
    return;
  }

  for (const bucket of matches) {
    lines.push(`MATCH ${formatBucketKey(bucket.key)} ${bucket.typescript}`);
  }

  for (const bucket of onlyTypeScript) {
    if (bucket.typescriptRust === 0) {
      lines.push(`ONLY_TS ${formatBucketKey(bucket.key)} ${bucket.typescript}`);
    } else {
      lines.push(
        `DIFF ${formatBucketKey(bucket.key)} TypeScript=${bucket.typescript} typescript-rust=${bucket.typescriptRust}`,
      );
    }
  }

  for (const bucket of onlyTypeScriptRust) {
    if (bucket.typescript === 0) {
      lines.push(`ONLY_RUST ${formatBucketKey(bucket.key)} ${bucket.typescriptRust}`);
    } else {
      lines.push(
        `DIFF ${formatBucketKey(bucket.key)} TypeScript=${bucket.typescript} typescript-rust=${bucket.typescriptRust}`,
      );
    }
  }
}

function formatBucketKey(key: string): string {
  const parts = key.split(' :: ');
  if (parts.length === 1) {
    return parts[0];
  }
  if (parts.length === 2) {
    return `${parts[0]} ${parts[1]}`;
  }
  return `${parts[0]} ${parts[1]} ${parts[2]}`;
}

try {
  main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`${message}\n`);
  process.exit(1);
}
