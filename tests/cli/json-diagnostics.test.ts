import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { expect, test } from 'vitest';

import { surge, tempProject } from '../harness/cli.ts';

const TSCONFIG = '{ "compilerOptions": {}, "include": ["src/**/*.ts"] }';

async function runCli(args: string[]): Promise<{ stdout: string; stderr: string }> {
	const result = await surge(args, { cwd: tmpdir() });
	expect([0, 2], `surge exited with ${result.exitCode}\n${result.stderr}`).toContain(
		result.exitCode,
	);
	return result;
}

function project(source: string): string {
	const root = tempProject({ 'tsconfig.json': TSCONFIG, 'src/index.ts': source });
	return join(root, 'tsconfig.json');
}

test('cli_diagnostics_json_single_file_positional', async () => {
	const root = tempProject({ 'index.ts': 'let value: string = 123;' });
	const { stdout, stderr } = await runCli(['--format', 'json', join(root, 'index.ts')]);

	expect(stderr).toBe('');
	const parsed = JSON.parse(stdout);
	expect(parsed.diagnostics).toHaveLength(1);
	const diagnostic = parsed.diagnostics[0];
	expect(diagnostic.code).toBe('TS2322');
	expect(diagnostic.fileName.endsWith('index.ts')).toBe(true);
	expect(diagnostic.message).not.toBe('');
	expect(typeof diagnostic.span.start).toBe('number');
	expect(typeof diagnostic.span.end).toBe('number');
	expect(diagnostic.line).toBeGreaterThanOrEqual(1);
	expect(diagnostic.column).toBeGreaterThanOrEqual(1);
});

test('cli_diagnostics_json_project', async () => {
	const { stdout, stderr } = await runCli([
		'--project',
		project('let value: string = 123;'),
		'--format',
		'json',
	]);

	expect(stderr).toBe('');
	const parsed = JSON.parse(stdout);
	expect(parsed.diagnostics).toHaveLength(1);
	const diagnostic = parsed.diagnostics[0];
	expect(diagnostic.code).toBe('TS2322');
	expect(diagnostic.fileName.replaceAll('\\', '/')).toBe('src/index.ts');
	expect(typeof diagnostic.span.start).toBe('number');
	expect(typeof diagnostic.span.end).toBe('number');
	expect(diagnostic.line).toBeGreaterThanOrEqual(1);
	expect(diagnostic.column).toBeGreaterThanOrEqual(1);
});

test('cli_diagnostics_json_includes_code_file_message', async () => {
	const { stdout, stderr } = await runCli([
		'--project',
		project('let value: string = 123;'),
		'--format',
		'json',
	]);

	expect(stderr).toBe('');
	const diagnostic = JSON.parse(stdout).diagnostics[0];
	expect(diagnostic.code).toBe('TS2322');
	expect(diagnostic.fileName.replaceAll('\\', '/')).toBe('src/index.ts');
	expect(diagnostic.message).not.toBe('');
});

test('cli_diagnostics_json_includes_span_when_available', async () => {
	const { stdout, stderr } = await runCli([
		'--project',
		project('let value: string = 123;'),
		'--format',
		'json',
	]);

	expect(stderr).toBe('');
	const diagnostic = JSON.parse(stdout).diagnostics[0];
	expect(typeof diagnostic.span.start).toBe('number');
	expect(typeof diagnostic.span.end).toBe('number');
});

test('cli_diagnostics_json_includes_line_column_when_available', async () => {
	const { stdout, stderr } = await runCli([
		'--project',
		project('let value: string = 123;'),
		'--format',
		'json',
	]);

	expect(stderr).toBe('');
	const diagnostic = JSON.parse(stdout).diagnostics[0];
	expect(diagnostic.line).toBeGreaterThanOrEqual(1);
	expect(diagnostic.column).toBeGreaterThanOrEqual(1);
});

test('cli_diagnostics_json_empty_diagnostics', async () => {
	const { stdout, stderr } = await runCli([
		'--project',
		project('let value: string = "ok";'),
		'--format',
		'json',
	]);

	expect(stderr).toBe('');
	expect(JSON.parse(stdout).diagnostics).toHaveLength(0);
});

test('cli_format_json_does_not_require_compat_report', async () => {
	const { stdout, stderr } = await runCli([
		'--project',
		project('let value: string = 123;'),
		'--format',
		'json',
	]);

	expect(stderr).toBe('');
	expect(() => JSON.parse(stdout)).not.toThrow();
});

test('cli_diagnostics_json_single_file_positional_with_show_spans_policy', async () => {
	const tsconfig = project('let value: string = 123;');
	const plain = await runCli(['--project', tsconfig, '--format', 'json']);
	const spans = await runCli(['--project', tsconfig, '--showSpans', '--format', 'json']);

	expect(plain.stderr).toBe('');
	expect(spans.stderr).toBe('');
	expect(spans.stdout).toBe(plain.stdout);
});

test('cli_diagnostics_json_single_file_normal_output_unchanged', async () => {
	const root = tempProject({ 'index.ts': 'let value: string = 123;' });
	const { stdout, stderr } = await runCli([join(root, 'index.ts')]);

	expect(stderr).toBe('');
	expect(stdout).toContain('TS2322');
	expect(stdout.trimStart().startsWith('{')).toBe(false);
});
