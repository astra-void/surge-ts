import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';

import { expect, onTestFinished } from 'vitest';

import { formatDiagnostic, parsePlainDiagnostics } from './diagnostics.ts';
import { surgeBinary, tscBinary } from './paths.ts';
import { type ProcessResult, crashDescription, runProcess } from './run.ts';

/**
 * Writes `files` into a fresh directory outside the workspace, removed when
 * the current test finishes.
 */
export function tempProject(files: Record<string, string>): string {
	const root = join(tmpdir(), `surge-vitest-${process.pid}`);
	mkdirSync(root, { recursive: true });
	const dir = mkdtempSync(join(root, 'cli-'));
	for (const [name, text] of Object.entries(files)) {
		const target = join(dir, name);
		mkdirSync(dirname(target), { recursive: true });
		writeFileSync(target, text);
	}
	onTestFinished(() => rmSync(dir, { recursive: true, force: true }));
	return dir;
}

export function surge(
	args: string[],
	options: { cwd: string; env?: NodeJS.ProcessEnv },
): Promise<ProcessResult> {
	return runProcess(surgeBinary(), args, options.cwd, { ...process.env, ...options.env });
}

export function tsc(args: string[], options: { cwd: string }): Promise<ProcessResult> {
	return runProcess(tscBinary(), args, options.cwd);
}

export function expectNoCrash(result: ProcessResult): void {
	const crash = crashDescription(result);
	expect(crash, `${result.stdout}${result.stderr}`).toBeNull();
}

/**
 * Runs surge and tsc with the same arguments in `cwd` and requires the same
 * diagnostics (file, line, column, code; plus message text when asked). tsc
 * also gets `--noEmit`, which surge has no flag for: surge never emits, and a
 * workspace fixture without `noEmit` must not get .js written beside it.
 */
export async function expectSameDiagnosticsAsTsc(
	cwd: string,
	args: string[],
	options: { messages?: boolean } = {},
): Promise<void> {
	const [surgeRun, tscRun] = await Promise.all([
		surge([...args, '--pretty', 'false'], { cwd }),
		tsc([...args, '--pretty', 'false', '--noEmit'], { cwd }),
	]);
	expectNoCrash(surgeRun);
	const fields = { messages: options.messages ?? false, spans: false };
	const render = (run: ProcessResult) =>
		parsePlainDiagnostics(run.stdout, cwd)
			.map((diagnostic) => formatDiagnostic(diagnostic, fields))
			.sort();
	expect(
		render(surgeRun),
		`surge output:\n${surgeRun.stdout}${surgeRun.stderr}\ntsc output:\n${tscRun.stdout}${tscRun.stderr}`,
	).toEqual(render(tscRun));
}
