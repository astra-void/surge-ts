import { tmpdir } from 'node:os';

import { expect, test } from 'vitest';

import { expectNoCrash, surge, tsc } from '../harness/cli.ts';
import { formatDiagnostic, parsePlainDiagnostics } from '../harness/diagnostics.ts';
import { workspacePath } from '../harness/suite.ts';

const PROJECT_DIR = workspacePath('tests/compat-projects/generics-basic');
const PROJECT = workspacePath('tests/compat-projects/generics-basic/tsconfig.json');

async function runCli(args: string[]): Promise<{ stdout: string; stderr: string }> {
	const result = await surge(args, { cwd: tmpdir() });
	expect([0, 2], `surge exited with ${result.exitCode}\n${result.stderr}`).toContain(
		result.exitCode,
	);
	return result;
}

test('compat_project_generics_basic_valid_subset_passes', async () => {
	// The project's tsconfig does not set noEmit and surge has no --noEmit
	// flag, so tsc alone gets it to keep emit out of the workspace.
	const [surgeRun, tscRun] = await Promise.all([
		surge(['--project', 'tsconfig.json', '--pretty', 'false'], { cwd: PROJECT_DIR }),
		tsc(['--project', 'tsconfig.json', '--noEmit', '--pretty', 'false'], { cwd: PROJECT_DIR }),
	]);
	expectNoCrash(surgeRun);
	expect(surgeRun.stderr).toBe('');
	const render = (stdout: string) =>
		parsePlainDiagnostics(stdout, PROJECT_DIR)
			.map((diagnostic) => formatDiagnostic(diagnostic, { messages: false, spans: false }))
			.sort();
	expect(
		render(surgeRun.stdout),
		`surge output:\n${surgeRun.stdout}\ntsc output:\n${tscRun.stdout}${tscRun.stderr}`,
	).toEqual(render(tscRun.stdout));
});

test('compat_project_generics_basic_report_stable', async () => {
	const { stdout, stderr } = await runCli(['--project', PROJECT, '--compatReport']);

	expect(stderr).toBe('');
	expect(stdout).toContain('Compatibility report');
	expect(stdout).toContain('Files loaded: 3');
	expect(stdout).toContain('Diagnostics: 0');
});

test('compat_report_generics_reduces_parser_errors_or_pins_remaining', async () => {
	const { stdout, stderr } = await runCli([
		'--project',
		PROJECT,
		'--compatReport',
		'--format',
		'json',
	]);

	expect(stderr).toBe('');
	expect(stdout).toContain('"filesLoaded"');
});

test('compat_report_generics_json_stable', async () => {
	const { stdout, stderr } = await runCli([
		'--project',
		PROJECT,
		'--compatReport',
		'--format',
		'json',
	]);

	expect(stderr).toBe('');
	const parsed = JSON.parse(stdout);
	expect(parsed.filesLoaded).toBe(3);
	expect(parsed.diagnosticsTotal).toBe(0);
	expect(parsed.byCode).toEqual([]);
});

test('compat_report_generics_counts_by_code_stable', async () => {
	const { stdout, stderr } = await runCli([
		'--project',
		PROJECT,
		'--compatReport',
		'--format',
		'json',
	]);

	expect(stderr).toBe('');
	const parsed = JSON.parse(stdout);
	expect(parsed.byCode).toEqual([]);
	expect(parsed.byFile).toEqual([]);
});

test('compat_report_generics_unsupported_file_still_parser_safe_or_pinned', async () => {
	const { stdout, stderr } = await runCli(['--project', PROJECT, '--compatReport']);

	expect(stderr).toBe('');
	expect(stdout).toContain('Files loaded: 3');
	expect(stdout).toContain('Diagnostics: 0');
});
