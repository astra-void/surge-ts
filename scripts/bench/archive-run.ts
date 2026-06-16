#!/usr/bin/env tsx

import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(import.meta.url);
const scriptDir = path.dirname(scriptPath);
const workspaceRoot = path.resolve(scriptDir, '../..');

export type ArchiveArgs = {
  bench: boolean;
  realAuthKit: boolean;
  label: string | null;
  out: string | null;
  dryRun: boolean;
};

export type PlannedStep = {
  name: string;
  executable: string;
  argv: string[];
  command: string;
  logFile: string;
  jsonFile: string | null;
};

export type CommandRun = {
  name: string;
  command: string;
  exitCode: number | null;
  ok: boolean;
  logFile: string;
  jsonFile: string | null;
};

export type GitInfo = {
  branch: string | null;
  commit: string | null;
  dirty: boolean | null;
};

export type BenchMedians = {
  project: string;
  medians: Record<string, number | null>;
};

export type AuthKitCounts = {
  typescriptTotal: number | null;
  typescriptRustTotal: number | null;
  codeCountMatch: boolean | null;
};

export type ArchiveSummary = {
  timestamp: string;
  label: string | null;
  outDir: string;
  git: GitInfo;
  commands: CommandRun[];
  medians: BenchMedians[];
  authKit: AuthKitCounts | null;
  parseWarnings: string[];
};

const BENCH_TOOLS = ['tsc', 'tsgo', 'tsgo-singleThreaded', 'ts-rust'] as const;

export function parseArgs(argv: string[]): ArchiveArgs {
  const parsed: ArchiveArgs = {
    bench: false,
    realAuthKit: false,
    label: null,
    out: null,
    dryRun: false,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--') continue;
    if (arg === '--bench') {
      parsed.bench = true;
    } else if (arg === '--real-auth-kit') {
      parsed.realAuthKit = true;
    } else if (arg === '--label') {
      parsed.label = argv[++i] ?? null;
    } else if (arg === '--out') {
      parsed.out = argv[++i] ?? null;
    } else if (arg === '--dryRun' || arg === '--dry-run') {
      parsed.dryRun = true;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return parsed;
}

export function sanitizeLabel(label: string): string {
  const cleaned = label
    .trim()
    .replace(/[^A-Za-z0-9._-]+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^[-.]+|[-.]+$/g, '');
  return cleaned;
}

export function timestampSlug(date: Date): string {
  return date.toISOString().replace(/\.\d+Z$/, '').replace(/:/g, '-');
}

export function defaultOutDir(root: string, timestamp: string): string {
  return path.join(root, '.bench', 'runs', timestamp);
}

export function buildPlan(
  modes: { bench: boolean; realAuthKit: boolean },
  outDir: string,
): PlannedStep[] {
  const steps: PlannedStep[] = [];

  if (modes.bench) {
    const jsonFile = path.join(outDir, 'bench-compilers.json');
    const argv = ['run', 'bench:compilers', '--', '--preset', 'current', '--json', jsonFile];
    steps.push({
      name: 'bench-compilers',
      executable: 'pnpm',
      argv,
      command: ['pnpm', ...argv].join(' '),
      logFile: path.join(outDir, 'bench-compilers.txt'),
      jsonFile,
    });
  }

  if (modes.realAuthKit) {
    const argv = ['run', 'real:auth-kit'];
    steps.push({
      name: 'real-auth-kit',
      executable: 'pnpm',
      argv,
      command: ['pnpm', ...argv].join(' '),
      logFile: path.join(outDir, 'real-auth-kit.txt'),
      jsonFile: null,
    });
  }

  return steps;
}

export function extractBenchMedians(benchJson: unknown): BenchMedians[] {
  if (!Array.isArray(benchJson)) {
    return [];
  }
  const out: BenchMedians[] = [];
  for (const entry of benchJson) {
    if (!entry || typeof entry !== 'object') continue;
    const record = entry as { project?: unknown; stats?: Record<string, unknown> };
    const project = typeof record.project === 'string' ? record.project : 'unknown';
    const medians: Record<string, number | null> = {};
    for (const tool of BENCH_TOOLS) {
      const stat = record.stats?.[tool];
      if (stat && typeof stat === 'object' && typeof (stat as { median?: unknown }).median === 'number') {
        medians[tool] = (stat as { median: number }).median;
      } else {
        medians[tool] = null;
      }
    }
    out.push({ project, medians });
  }
  return out;
}

export function extractAuthKitCounts(markdown: string): AuthKitCounts | null {
  const tsTotal = matchNumber(markdown, /TypeScript total diagnostics:\s*(\d+)/);
  const rustTotal = matchNumber(markdown, /typescript-rust total diagnostics:\s*(\d+)/);
  const matchLine = markdown.match(/code-count match:\s*(yes|no)/i);

  if (tsTotal === null && rustTotal === null && !matchLine) {
    return null;
  }

  return {
    typescriptTotal: tsTotal,
    typescriptRustTotal: rustTotal,
    codeCountMatch: matchLine ? matchLine[1].toLowerCase() === 'yes' : null,
  };
}

function matchNumber(text: string, pattern: RegExp): number | null {
  const match = text.match(pattern);
  return match ? Number(match[1]) : null;
}

export function buildSummary(input: {
  timestamp: string;
  label: string | null;
  outDir: string;
  git: GitInfo;
  commands: CommandRun[];
  medians: BenchMedians[];
  authKit: AuthKitCounts | null;
  parseWarnings: string[];
}): ArchiveSummary {
  return {
    timestamp: input.timestamp,
    label: input.label,
    outDir: input.outDir,
    git: input.git,
    commands: input.commands,
    medians: input.medians,
    authKit: input.authKit,
    parseWarnings: input.parseWarnings,
  };
}

export function renderSummaryMarkdown(summary: ArchiveSummary): string {
  const titleSuffix = summary.label ? ` — ${summary.label}` : '';
  const lines: string[] = [];

  lines.push(`# Benchmark Archive — ${summary.timestamp}${titleSuffix}`, '');

  const gitParts: string[] = [];
  if (summary.git.branch) gitParts.push(summary.git.branch);
  if (summary.git.commit) gitParts.push(summary.git.commit.slice(0, 12));
  let gitLine = gitParts.length > 0 ? gitParts.join(' @ ') : 'n/a';
  if (summary.git.dirty !== null) gitLine += ` (${summary.git.dirty ? 'dirty' : 'clean'})`;
  lines.push(`- Git: ${gitLine}`);
  lines.push(`- Output directory: \`${summary.outDir}\``);
  lines.push('');

  lines.push('## Commands', '');
  lines.push('| command | status | exit code | log |');
  lines.push('| --- | --- | ---: | --- |');
  for (const cmd of summary.commands) {
    const status = cmd.ok ? 'pass' : 'fail';
    lines.push(`| \`${cmd.command}\` | ${status} | ${cmd.exitCode ?? 'n/a'} | \`${cmd.logFile}\` |`);
  }
  lines.push('');

  if (summary.medians.length > 0) {
    lines.push('## Benchmark Medians', '');
    lines.push(`| project | ${BENCH_TOOLS.join(' | ')} |`);
    lines.push(`| --- | ${BENCH_TOOLS.map(() => '---:').join(' | ')} |`);
    for (const entry of summary.medians) {
      const cells = BENCH_TOOLS.map((tool) => {
        const value = entry.medians[tool];
        return value === null || value === undefined ? 'n/a' : `${value.toFixed(2)}s`;
      });
      lines.push(`| ${entry.project} | ${cells.join(' | ')} |`);
    }
    lines.push('');
  }

  if (summary.authKit) {
    lines.push('## Auth-Kit Diagnostics', '');
    lines.push(`- TypeScript total: ${summary.authKit.typescriptTotal ?? 'n/a'}`);
    lines.push(`- typescript-rust total: ${summary.authKit.typescriptRustTotal ?? 'n/a'}`);
    lines.push(
      `- code-count match: ${
        summary.authKit.codeCountMatch === null ? 'n/a' : summary.authKit.codeCountMatch ? 'yes' : 'no'
      }`,
    );
    lines.push('');
  }

  if (summary.parseWarnings.length > 0) {
    lines.push('## Parse Notes', '');
    for (const warning of summary.parseWarnings) {
      lines.push(`- ${warning}`);
    }
    lines.push('');
  }

  return lines.join('\n');
}

function getGitInfo(): GitInfo {
  const git = (args: string[]): string | null => {
    try {
      const res = spawnSync('git', args, { cwd: workspaceRoot, encoding: 'utf8' });
      if (res.status !== 0 || typeof res.stdout !== 'string') return null;
      return res.stdout.trim();
    } catch {
      return null;
    }
  };

  const branch = git(['rev-parse', '--abbrev-ref', 'HEAD']);
  const commit = git(['rev-parse', 'HEAD']);
  const statusRaw = git(['status', '--porcelain']);

  return {
    branch: branch || null,
    commit: commit || null,
    dirty: statusRaw === null ? null : statusRaw.length > 0,
  };
}

function runStep(step: PlannedStep): CommandRun {
  const res = spawnSync(step.executable, step.argv, {
    cwd: workspaceRoot,
    encoding: 'utf8',
    maxBuffer: 50 * 1024 * 1024,
    env: {
      ...process.env,
      npm_config_cache: process.env.npm_config_cache ?? path.join(os.tmpdir(), 'npm-cache'),
    },
  });

  const stdout = res.stdout ?? '';
  const stderr = res.stderr ?? '';
  const errorNote = res.error ? `\n[spawn error] ${res.error.message}\n` : '';
  writeFileSync(step.logFile, `$ ${step.command}\n\n${stdout}${stderr}${errorNote}`);

  const exitCode = res.error ? null : res.status;
  return {
    name: step.name,
    command: step.command,
    exitCode,
    ok: exitCode === 0,
    logFile: path.relative(workspaceRoot, step.logFile),
    jsonFile: step.jsonFile ? path.relative(workspaceRoot, step.jsonFile) : null,
  };
}

function main(argv = process.argv.slice(2)): void {
  const args = parseArgs(argv);

  const modes = { bench: args.bench, realAuthKit: args.realAuthKit };
  if (!modes.bench && !modes.realAuthKit) {
    modes.bench = true;
  }

  const timestamp = timestampSlug(new Date());
  const outDir = args.out
    ? path.resolve(workspaceRoot, args.out)
    : defaultOutDir(workspaceRoot, timestamp);
  const label = args.label ? sanitizeLabel(args.label) : null;

  const plan = buildPlan(modes, outDir);

  console.log(`Output directory: ${outDir}`);
  for (const step of plan) {
    console.log(`  ${step.name}: ${step.command}`);
  }

  if (args.dryRun) {
    console.log('Dry run: no commands executed, no files written.');
    return;
  }

  mkdirSync(outDir, { recursive: true });

  const commands: CommandRun[] = [];
  let anyFailed = false;
  for (const step of plan) {
    console.log(`\nRunning ${step.name}: ${step.command}`);
    const result = runStep(step);
    if (!result.ok) anyFailed = true;
    console.log(`  exit code: ${result.exitCode ?? 'n/a'} (log: ${result.logFile})`);
    commands.push(result);
  }

  const parseWarnings: string[] = [];

  const medians: BenchMedians[] = [];
  const benchStep = plan.find((step) => step.name === 'bench-compilers');
  if (benchStep?.jsonFile) {
    if (existsSync(benchStep.jsonFile)) {
      try {
        const parsed = JSON.parse(readFileSync(benchStep.jsonFile, 'utf8'));
        medians.push(...extractBenchMedians(parsed));
      } catch (error) {
        parseWarnings.push(`Failed to parse bench JSON: ${(error as Error).message}`);
      }
    } else if (commands.some((cmd) => cmd.name === 'bench-compilers' && cmd.ok)) {
      parseWarnings.push('Bench JSON was not produced despite a successful run.');
    }
  }

  let authKit: AuthKitCounts | null = null;
  if (modes.realAuthKit) {
    const measurementPath = path.join(workspaceRoot, '.bench', 'auth-kit-measurement.md');
    if (existsSync(measurementPath)) {
      authKit = extractAuthKitCounts(readFileSync(measurementPath, 'utf8'));
      if (!authKit) {
        parseWarnings.push('auth-kit-measurement.md present but diagnostic counts could not be parsed.');
      }
    } else {
      parseWarnings.push('auth-kit-measurement.md not found; diagnostic counts unavailable.');
    }
  }

  const summary = buildSummary({
    timestamp,
    label,
    outDir,
    git: getGitInfo(),
    commands,
    medians,
    authKit,
    parseWarnings,
  });

  writeFileSync(path.join(outDir, 'summary.json'), `${JSON.stringify(summary, null, 2)}\n`);
  writeFileSync(path.join(outDir, 'summary.md'), `${renderSummaryMarkdown(summary)}\n`);

  console.log(`\nWrote summary.json and summary.md to ${outDir}`);

  if (anyFailed) {
    process.exitCode = 1;
  }
}

const invokedDirectly =
  Boolean(process.argv[1]) && path.resolve(process.argv[1]) === scriptPath;
if (invokedDirectly) {
  main();
}
