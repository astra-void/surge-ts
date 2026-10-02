import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';

import {
	type Diagnostic,
	parsePlainDiagnostics,
	parsePrettyDiagnostics,
} from './diagnostics.ts';
import type { Fixture } from './fixture.ts';
import { WORKSPACE_ROOT, surgeBinary, tscBinary, tscVersion } from './paths.ts';

export type ProcessResult = {
	exitCode: number | null;
	signal: NodeJS.Signals | null;
	stdout: string;
	stderr: string;
};

export type CheckerRun = ProcessResult & { diagnostics: Diagnostic[] };

// Fixture projects live outside the workspace so neither checker walks up into
// the repository's node_modules (and its @types) while resolving.
const SCRATCH_ROOT = join(tmpdir(), `surge-vitest-${process.pid}`);
const TSC_CACHE_DIR = join(WORKSPACE_ROOT, 'node_modules/.cache/surge-vitest/tsc');

export function runProcess(
	command: string,
	args: string[],
	cwd: string,
	env: NodeJS.ProcessEnv = process.env,
): Promise<ProcessResult> {
	return new Promise((resolvePromise, reject) => {
		const child = spawn(command, args, { cwd, env, stdio: ['ignore', 'pipe', 'pipe'] });
		const stdout: Buffer[] = [];
		const stderr: Buffer[] = [];
		child.stdout.on('data', (chunk: Buffer) => stdout.push(chunk));
		child.stderr.on('data', (chunk: Buffer) => stderr.push(chunk));
		child.on('error', reject);
		child.on('close', (exitCode, signal) => {
			resolvePromise({
				exitCode,
				signal,
				stdout: Buffer.concat(stdout).toString('utf8'),
				stderr: Buffer.concat(stderr).toString('utf8'),
			});
		});
	});
}

export function tsconfigFor(fixture: Fixture): string {
	return `${JSON.stringify(
		{
			compilerOptions: { ...fixture.compilerOptions, noEmit: true },
			include: ['**/*'],
		},
		null,
		2,
	)}\n`;
}

export function materialize(fixture: Fixture): string {
	mkdirSync(SCRATCH_ROOT, { recursive: true });
	const dir = mkdtempSync(join(SCRATCH_ROOT, `${fixture.name.slice(0, 60)}-`));
	writeFileSync(join(dir, 'tsconfig.json'), tsconfigFor(fixture));
	for (const file of fixture.files) {
		const target = join(dir, file.name);
		mkdirSync(dirname(target), { recursive: true });
		writeFileSync(target, file.text);
	}
	return dir;
}

export function removeProject(dir: string): void {
	rmSync(dir, { recursive: true, force: true });
}

function parse(output: string, dir: string, pretty: boolean): Diagnostic[] {
	return pretty ? parsePrettyDiagnostics(output, dir) : parsePlainDiagnostics(output, dir);
}

export async function runSurge(
	fixture: Fixture,
	dir: string,
	pretty: boolean,
): Promise<CheckerRun> {
	const result = await runProcess(
		surgeBinary(),
		['-p', 'tsconfig.json', '--pretty', String(pretty), ...fixture.surgeArgs],
		dir,
	);
	return { ...result, diagnostics: parse(result.stdout, dir, pretty) };
}

/**
 * tsc output depends only on the compiler version and the project's content, so
 * it is cached under node_modules/.cache keyed by exactly those inputs.
 */
export async function runTsc(
	fixture: Fixture,
	dir: string,
	pretty: boolean,
): Promise<CheckerRun> {
	const key = createHash('sha256')
		.update(
			JSON.stringify({
				version: tscVersion(),
				pretty,
				tsconfig: tsconfigFor(fixture),
				files: fixture.files,
			}),
		)
		.digest('hex');
	const cachePath = join(TSC_CACHE_DIR, key.slice(0, 2), `${key}.json`);

	let result: ProcessResult;
	try {
		result = JSON.parse(readFileSync(cachePath, 'utf8')) as ProcessResult;
	} catch {
		result = await runProcess(
			tscBinary(),
			['-p', 'tsconfig.json', '--pretty', String(pretty)],
			dir,
		);
		if (result.signal === null && (result.exitCode ?? 99) <= 2) {
			mkdirSync(dirname(cachePath), { recursive: true });
			writeFileSync(cachePath, JSON.stringify(result));
		}
	}
	return { ...result, diagnostics: parse(result.stdout, dir, pretty) };
}

/** A surge run is broken, not merely divergent, when it panics or is killed. */
export function crashDescription(run: ProcessResult): string | null {
	if (run.signal !== null) return `killed by ${run.signal}`;
	if (run.exitCode === null || run.exitCode > 2) return `exited with ${run.exitCode}`;
	if (run.stderr.includes('panicked at')) return 'panicked';
	return null;
}
