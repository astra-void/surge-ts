// A deferred intersection merge that reaches itself while it is being merged
// must terminate. Runs tests/compat-projects/lazy-intersection-reentry-basic,
// reduced from `@xata.io/client`: before the fix the checker blocked forever on
// the merge's own `OnceLock`, without using CPU, so the child runs under a
// deadline and is killed rather than left to hang the test run. The reduction
// needs the bundled lib (`Readonly`, `Omit`, `Awaited`, `ReturnType`), which is
// why it runs the CLI in project mode.

import { spawn } from 'node:child_process';

import { expect, test } from 'vitest';

import { tsc } from '../harness/cli.ts';
import { formatDiagnostic, parsePlainDiagnostics } from '../harness/diagnostics.ts';
import { surgeBinary } from '../harness/paths.ts';
import { workspacePath } from '../harness/suite.ts';

const PROJECT_DIR = workspacePath('tests/compat-projects/lazy-intersection-reentry-basic');
const DEADLINE_MS = 60_000;

function surgeWithDeadline(
	args: string[],
	cwd: string,
): Promise<{ exitCode: number | null; stdout: string; timedOut: boolean }> {
	return new Promise((resolvePromise, reject) => {
		const child = spawn(surgeBinary(), args, { cwd, stdio: ['ignore', 'pipe', 'ignore'] });
		const stdout: Buffer[] = [];
		let timedOut = false;
		const timer = setTimeout(() => {
			timedOut = true;
			child.kill('SIGKILL');
		}, DEADLINE_MS);
		child.stdout.on('data', (chunk: Buffer) => stdout.push(chunk));
		child.on('error', (error) => {
			clearTimeout(timer);
			reject(error);
		});
		child.on('close', (exitCode) => {
			clearTimeout(timer);
			resolvePromise({ exitCode, stdout: Buffer.concat(stdout).toString('utf8'), timedOut });
		});
	});
}

test('self_reaching_intersection_merge_terminates', async () => {
	const args = ['--project', 'tsconfig.json', '--pretty', 'false'];
	const [surgeRun, tscRun] = await Promise.all([
		surgeWithDeadline(args, PROJECT_DIR),
		tsc(args, { cwd: PROJECT_DIR }),
	]);

	expect(
		surgeRun.timedOut,
		'checking a self-reaching intersection merge did not terminate',
	).toBe(false);
	expect(surgeRun.exitCode, surgeRun.stdout).toBe(2);
	const render = (stdout: string) =>
		parsePlainDiagnostics(stdout, PROJECT_DIR)
			.map((diagnostic) => formatDiagnostic(diagnostic, { messages: false, spans: false }))
			.sort();
	expect(
		render(surgeRun.stdout),
		`surge output:\n${surgeRun.stdout}\ntsc output:\n${tscRun.stdout}${tscRun.stderr}`,
	).toEqual(render(tscRun.stdout));
});
