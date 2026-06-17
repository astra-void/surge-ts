import assert from 'node:assert/strict';
import path from 'node:path';
import test from 'node:test';

import type { SpawnSyncReturns } from 'node:child_process';

import {
  authKitCandidateRoots,
  type MeasuredCommandResult,
  outputPathsForProject,
  parseArgs,
  parsePeakRssBytes,
  parseRustJobs,
  peakRssMb,
  projectNameFromPath,
  resolveProject,
  runMeasuredCommand,
  slugify,
  timeMeasurementForPlatform,
} from './measure-project';

const MACOS_TIME_REPORT = [
  '        0.55 real         0.42 user         0.12 sys',
  '           170999808  maximum resident set size',
  '                   0  average shared memory size',
  '               10712  page reclaims',
  '           167330320  peak memory footprint',
].join('\n');

const LINUX_TIME_REPORT = [
  '\tCommand being timed: "target/release/surge --project tsconfig.json"',
  '\tUser time (seconds): 0.40',
  '\tMaximum resident set size (kbytes): 166992',
  '\tExit status: 0',
].join('\n');

function fakeSpawn(
  overrides: Partial<SpawnSyncReturns<string>> = {},
): { spawn: typeof import('node:child_process').spawnSync; calls: Array<{ command: string; args: string[] }> } {
  const calls: Array<{ command: string; args: string[] }> = [];
  const spawn = ((command: string, args: string[]) => {
    calls.push({ command, args });
    return {
      pid: 1,
      output: [],
      stdout: '',
      stderr: '',
      status: 0,
      signal: null,
      ...overrides,
    } as SpawnSyncReturns<string>;
  }) as unknown as typeof import('node:child_process').spawnSync;
  return { spawn, calls };
}

test('parsePeakRssBytes reads macOS time -l bytes verbatim', () => {
  assert.equal(parsePeakRssBytes(MACOS_TIME_REPORT, 'macos-time'), 170999808);
});

test('parsePeakRssBytes converts Linux time -v kbytes to bytes', () => {
  assert.equal(parsePeakRssBytes(LINUX_TIME_REPORT, 'linux-time'), 166992 * 1024);
});

test('parsePeakRssBytes returns null when the field is absent', () => {
  assert.equal(parsePeakRssBytes('no rss here', 'macos-time'), null);
  assert.equal(parsePeakRssBytes('no rss here', 'linux-time'), null);
  assert.equal(parsePeakRssBytes(MACOS_TIME_REPORT, 'unavailable'), null);
});

test('peakRssMb rounds bytes to one decimal megabyte', () => {
  assert.equal(peakRssMb(null), null);
  assert.equal(peakRssMb(1024 * 1024), 1);
  assert.equal(peakRssMb(170999808), 163.1);
});

test('timeMeasurementForPlatform maps darwin/linux and rejects others', () => {
  assert.deepEqual(timeMeasurementForPlatform('darwin'), { flag: '-l', source: 'macos-time' });
  assert.deepEqual(timeMeasurementForPlatform('linux'), { flag: '-v', source: 'linux-time' });
  assert.equal(timeMeasurementForPlatform('win32'), null);
});

test('runMeasuredCommand parses macOS peak RSS and keeps child output clean', () => {
  const { spawn, calls } = fakeSpawn({ stdout: 'OK', stderr: 'Timings:\n  parsing: 1ms' });
  const result: MeasuredCommandResult = runMeasuredCommand('bin', ['--project', 'tsconfig.json'], {
    platform: 'darwin',
    spawn,
    timeBinaryExists: () => true,
    timeBinaryPath: '/usr/bin/time',
    makeReportPath: () => '/tmp/report',
    readReport: () => MACOS_TIME_REPORT,
    now: () => 0,
  });

  assert.equal(result.peakRssBytes, 170999808);
  assert.equal(result.peakRssSource, 'macos-time');
  assert.equal(result.status, 0);
  assert.equal(result.stdout, 'OK');
  assert.equal(result.stderr, 'Timings:\n  parsing: 1ms');
  // time is the spawned process; the measured command is passed after `-o file`.
  assert.deepEqual(calls[0], {
    command: '/usr/bin/time',
    args: ['-l', '-o', '/tmp/report', 'bin', '--project', 'tsconfig.json'],
  });
});

test('runMeasuredCommand parses Linux peak RSS', () => {
  const { spawn, calls } = fakeSpawn();
  const result = runMeasuredCommand('bin', ['--project', 'tsconfig.json'], {
    platform: 'linux',
    spawn,
    timeBinaryExists: () => true,
    makeReportPath: () => '/tmp/report',
    readReport: () => LINUX_TIME_REPORT,
  });

  assert.equal(result.peakRssBytes, 166992 * 1024);
  assert.equal(result.peakRssSource, 'linux-time');
  assert.equal(calls[0].args[0], '-v');
});

test('runMeasuredCommand falls back to direct execution when time is unavailable', () => {
  const { spawn, calls } = fakeSpawn({ stdout: 'direct', stderr: '' });
  const result = runMeasuredCommand('bin', ['--project', 'tsconfig.json'], {
    platform: 'darwin',
    spawn,
    timeBinaryExists: () => false,
  });

  assert.equal(result.peakRssBytes, null);
  assert.equal(result.peakRssSource, 'unavailable');
  assert.equal(result.stdout, 'direct');
  // No /usr/bin/time wrapper: the command runs directly.
  assert.deepEqual(calls[0], { command: 'bin', args: ['--project', 'tsconfig.json'] });
});

test('runMeasuredCommand still reports memory when the child command fails', () => {
  const { spawn } = fakeSpawn({ status: 2, stdout: 'partial', stderr: 'boom' });
  const result = runMeasuredCommand('bin', ['--project', 'tsconfig.json'], {
    platform: 'darwin',
    spawn,
    timeBinaryExists: () => true,
    makeReportPath: () => '/tmp/report',
    readReport: () => MACOS_TIME_REPORT,
  });

  assert.equal(result.status, 2);
  assert.equal(result.stdout, 'partial');
  assert.equal(result.stderr, 'boom');
  assert.equal(result.peakRssBytes, 170999808);
  assert.equal(result.peakRssSource, 'macos-time');
});

test('runMeasuredCommand marks memory unavailable when the report is unparseable', () => {
  const { spawn } = fakeSpawn();
  const result = runMeasuredCommand('bin', [], {
    platform: 'darwin',
    spawn,
    timeBinaryExists: () => true,
    makeReportPath: () => '/tmp/report',
    readReport: () => 'garbage with no rss',
  });

  assert.equal(result.peakRssBytes, null);
  assert.equal(result.peakRssSource, 'unavailable');
});

test('parseArgs reads all supported flags', () => {
  const parsed = parseArgs([
    '--project',
    '/abs/project/tsconfig.json',
    '--name',
    'My App',
    '--maxDiagnostics',
    '1000',
    '--rustJobs',
    '1,4',
    '--outDir',
    '/tmp/out',
    '--authKitFallback',
    '--allowMissing',
  ]);

  assert.equal(parsed.project, '/abs/project/tsconfig.json');
  assert.equal(parsed.name, 'My App');
  assert.equal(parsed.maxDiagnostics, 1000);
  assert.deepEqual(parsed.rustJobs, [1, 4]);
  assert.equal(parsed.outDir, '/tmp/out');
  assert.equal(parsed.authKitFallback, true);
  assert.equal(parsed.allowMissing, true);
});

test('parseArgs applies defaults', () => {
  const parsed = parseArgs(['--project', '/abs/project']);
  assert.equal(parsed.maxDiagnostics, 500);
  assert.deepEqual(parsed.rustJobs, [1, 4]);
  assert.equal(parsed.outDir, null);
  assert.equal(parsed.name, null);
  assert.equal(parsed.authKitFallback, false);
  assert.equal(parsed.allowMissing, false);
});

test('parseArgs rejects unknown arguments and bad values', () => {
  assert.throws(() => parseArgs(['--nope']), /Unknown argument/);
  assert.throws(() => parseArgs(['--maxDiagnostics', '0']), /positive integer/);
  assert.throws(() => parseArgs(['--project']), /Missing value/);
});

test('parseRustJobs parses, validates, and dedupes', () => {
  assert.deepEqual(parseRustJobs('1,4'), [1, 4]);
  assert.deepEqual(parseRustJobs('1,2,4'), [1, 2, 4]);
  assert.deepEqual(parseRustJobs(' 1 , 1 , 4 '), [1, 4]);
  assert.throws(() => parseRustJobs(''), /positive integers/);
  assert.throws(() => parseRustJobs('1,foo'), /positive integers/);
  assert.throws(() => parseRustJobs('0,1'), /positive integers/);
});

test('resolveProject uses an explicit tsconfig file path', () => {
  const resolved = resolveProject(
    { project: '/abs/project/tsconfig.json', authKitFallback: false },
    { workspaceRoot: '/repo', classify: (p) => (p === '/abs/project/tsconfig.json' ? 'file' : 'missing') },
  );
  assert.deepEqual(resolved, {
    root: '/abs/project',
    tsconfig: '/abs/project/tsconfig.json',
    attempted: ['/abs/project/tsconfig.json'],
  });
});

test('resolveProject finds tsconfig.json inside a directory', () => {
  const resolved = resolveProject(
    { project: '/abs/project', authKitFallback: false },
    {
      workspaceRoot: '/repo',
      classify: (p) => {
        if (p === '/abs/project') return 'dir';
        if (p === '/abs/project/tsconfig.json') return 'file';
        return 'missing';
      },
    },
  );
  assert.equal(resolved?.root, '/abs/project');
  assert.equal(resolved?.tsconfig, '/abs/project/tsconfig.json');
});

test('resolveProject returns null when directory has no tsconfig', () => {
  const resolved = resolveProject(
    { project: '/abs/project', authKitFallback: false },
    { workspaceRoot: '/repo', classify: (p) => (p === '/abs/project' ? 'dir' : 'missing') },
  );
  assert.equal(resolved, null);
});

test('resolveProject resolves relative project paths against cwd', () => {
  const resolved = resolveProject(
    { project: 'sub/tsconfig.json', authKitFallback: false },
    {
      workspaceRoot: '/repo',
      cwd: '/work',
      classify: (p) => (p === '/work/sub/tsconfig.json' ? 'file' : 'missing'),
    },
  );
  assert.equal(resolved?.tsconfig, '/work/sub/tsconfig.json');
  assert.equal(resolved?.root, '/work/sub');
});

test('explicit project wins over authKitFallback', () => {
  const candidateRoots = ['/candidate/auth-kit'];
  const resolved = resolveProject(
    { project: '/abs/project/tsconfig.json', authKitFallback: true },
    {
      workspaceRoot: '/repo',
      candidateRoots,
      classify: (p) =>
        p === '/abs/project/tsconfig.json' || p === '/candidate/auth-kit/tsconfig.json'
          ? 'file'
          : 'missing',
    },
  );
  assert.equal(resolved?.tsconfig, '/abs/project/tsconfig.json');
});

test('authKitCandidateRoots preserves env, secondary, and local candidates', () => {
  const withEnv = authKitCandidateRoots('/repo', { AUTH_KIT_PROJECT: '/env/auth-kit' });
  assert.deepEqual(withEnv, [
    '/env/auth-kit',
    path.resolve('/repo', '../../typescript/auth-project/auth-kit'),
    path.resolve('/repo', '.local-projects/auth-kit'),
  ]);

  const withoutEnv = authKitCandidateRoots('/repo', {});
  assert.deepEqual(withoutEnv, [
    path.resolve('/repo', '../../typescript/auth-project/auth-kit'),
    path.resolve('/repo', '.local-projects/auth-kit'),
  ]);
});

test('resolveProject auth-kit fallback walks candidate roots in order', () => {
  const candidateRoots = ['/first/auth-kit', '/second/auth-kit'];
  const resolved = resolveProject(
    { project: null, authKitFallback: true },
    {
      workspaceRoot: '/repo',
      candidateRoots,
      classify: (p) => (p === '/second/auth-kit/tsconfig.json' ? 'file' : 'missing'),
    },
  );
  assert.equal(resolved?.root, '/second/auth-kit');
  assert.deepEqual(resolved?.attempted, ['/first/auth-kit', '/second/auth-kit']);
});

test('resolveProject returns null without project or fallback', () => {
  assert.equal(
    resolveProject({ project: null, authKitFallback: false }, { workspaceRoot: '/repo' }),
    null,
  );
});

test('projectNameFromPath and slugify derive stable slugs', () => {
  assert.equal(projectNameFromPath('/abs/My Next App'), 'my-next-app');
  assert.equal(projectNameFromPath('/abs/auth-kit'), 'auth-kit');
  assert.equal(slugify('  trpc  '), 'trpc');
  assert.equal(slugify('@scope/pkg'), 'scope-pkg');
  assert.equal(slugify('***'), 'project');
});

test('outputPathsForProject names artifacts per rustJobs', () => {
  const outputs = outputPathsForProject('/out/my-app', [1, 2, 4]);
  assert.equal(outputs.measurementMd, path.join('/out/my-app', 'measurement.md'));
  assert.equal(outputs.oracleCompareTxt, path.join('/out/my-app', 'oracle-compare.txt'));
  assert.equal(outputs.oracleCompareJson, path.join('/out/my-app', 'oracle-compare.json'));
  assert.equal(outputs.compatReportJson, path.join('/out/my-app', 'compat-report.json'));
  assert.equal(outputs.timingsTxt, path.join('/out/my-app', 'timings.txt'));
  assert.deepEqual(
    outputs.jobs.map((job) => [job.jobs, path.basename(job.json), path.basename(job.svg)]),
    [
      [1, 'jobs1.json', 'jobs1.svg'],
      [2, 'jobs2.json', 'jobs2.svg'],
      [4, 'jobs4.json', 'jobs4.svg'],
    ],
  );
});
