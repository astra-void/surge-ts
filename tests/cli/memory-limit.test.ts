import { join } from 'node:path';

import { expect, test } from 'vitest';

import { surge, tempProject } from '../harness/cli.ts';

const ENV_VAR = 'SURGE_MAX_FOOTPRINT_MB';

function fixtureFile(): { dir: string; file: string } {
	const dir = tempProject({ 'index.ts': 'let value: string = 123;\n' });
	return { dir, file: join(dir, 'index.ts') };
}

function runWithLimit(limit: string | undefined, dir: string, file: string) {
	return surge([file], { cwd: dir, env: { [ENV_VAR]: limit } });
}

test('exceeding_the_limit_exits_137_with_a_message', async () => {
	const { dir, file } = fixtureFile();
	// Any live process already exceeds 1 MiB, so the synchronous first sample
	// fires before checking starts.
	const output = await runWithLimit('1', dir, file);
	expect(output.exitCode, `stderr: ${output.stderr}`).toBe(137);
	expect(output.stderr).toContain('surge: memory limit exceeded');
	expect(output.stderr).toContain(ENV_VAR);
	expect(output.stderr).toContain('last stage: before check');
	expect(output.stdout, 'no diagnostics once the guard fires').toBe('');
});

test('a_generous_limit_leaves_the_run_untouched', async () => {
	const { dir, file } = fixtureFile();
	const guarded = await runWithLimit('1048576', dir, file);
	const unguarded = await runWithLimit(undefined, dir, file);
	expect(guarded.exitCode).toBe(unguarded.exitCode);
	expect(guarded.exitCode, 'the fixture has one error').toBe(2);
	expect(guarded.stdout).toBe(unguarded.stdout);
	expect(guarded.stderr, 'the guard must stay silent when it does not fire').not.toContain(
		'memory limit',
	);
});

test('a_malformed_limit_is_a_usage_error', async () => {
	const { dir, file } = fixtureFile();
	for (const bad of ['0', '8g', '', 'abc']) {
		const output = await runWithLimit(bad, dir, file);
		expect(output.exitCode, `${JSON.stringify(bad)}: stderr: ${output.stderr}`).toBe(2);
		expect(output.stderr, `${JSON.stringify(bad)}: stderr: ${output.stderr}`).toContain(ENV_VAR);
		expect(output.stdout, `${JSON.stringify(bad)}: nothing must be checked`).toBe('');
	}
});
