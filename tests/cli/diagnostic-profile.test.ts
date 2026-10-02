import { join } from 'node:path';

import { expect, test } from 'vitest';

import { expectNoCrash, surge, tempProject, tsc } from '../harness/cli.ts';
import { formatDiagnostic, parsePlainDiagnostics } from '../harness/diagnostics.ts';

const SOURCE =
	'interface User { name: string; age?: number; }\nfunction acceptUser(u: User) {}\nacceptUser((1 as any as number) satisfies User);\n';

test('test_default_profile_is_tsc', async () => {
	const dir = tempProject({ 'index.ts': SOURCE });
	// surge has no --noEmit flag, so tsc gets its own to keep the scratch
	// directory free of emit.
	const [surgeRun, tscRun] = await Promise.all([
		surge(['index.ts', '--ignoreConfig', '--pretty', 'false'], { cwd: dir }),
		tsc(['index.ts', '--ignoreConfig', '--noEmit', '--pretty', 'false'], { cwd: dir }),
	]);
	expectNoCrash(surgeRun);
	const render = (stdout: string) =>
		parsePlainDiagnostics(stdout, dir)
			.map((diagnostic) => formatDiagnostic(diagnostic, { messages: true, spans: false }))
			.sort();
	expect(
		render(surgeRun.stdout),
		`surge output:\n${surgeRun.stdout}${surgeRun.stderr}\ntsc output:\n${tscRun.stdout}${tscRun.stderr}`,
	).toEqual(render(tscRun.stdout));
});

test('test_native_profile_suppresses_cascade', async () => {
	const dir = tempProject({ 'index.ts': SOURCE });
	const result = await surge(
		[join(dir, 'index.ts'), '--ignoreConfig', '--diagnosticProfile', 'native'],
		{ cwd: dir },
	);

	expect(result.stdout).toContain('does not satisfy the expected type');
	expect(result.stdout).not.toContain('is not assignable to parameter of type');
});

test('test_single_file_jobs_are_rejected_cleanly', async () => {
	const dir = tempProject({ 'index.ts': 'const value: string = 1;' });
	const result = await surge([join(dir, 'index.ts'), '--ignoreConfig', '--jobs', '4'], {
		cwd: dir,
	});

	expect(result.exitCode).not.toBe(0);
	expect(result.stderr).toContain('--jobs is only supported with --project');
});
