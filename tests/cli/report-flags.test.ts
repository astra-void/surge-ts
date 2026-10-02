import { existsSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { expect, test } from 'vitest';

import { surge, tempProject } from '../harness/cli.ts';
import type { ProcessResult } from '../harness/run.ts';

const TSCONFIG = '{ "compilerOptions": {}, "include": ["src/**/*.ts"] }';

function runCliRaw(args: string[]): Promise<ProcessResult> {
	return surge(args, { cwd: tmpdir() });
}

async function runCli(args: string[]): Promise<ProcessResult> {
	const result = await runCliRaw(args);
	expect([0, 2], `surge exited with ${result.exitCode}\nstderr: ${result.stderr}`).toContain(
		result.exitCode,
	);
	return result;
}

function fixtureProject(): { root: string; project: string } {
	const root = tempProject({ 'tsconfig.json': TSCONFIG, 'src/index.ts': 'let value: string = 123;' });
	return { root, project: join(root, 'tsconfig.json') };
}

function readJson(path: string) {
	return JSON.parse(readFileSync(path, 'utf8'));
}

test('default_concise_output_has_no_new_noise', async () => {
	const { project } = fixtureProject();
	const { stdout, stderr } = await runCli(['--project', project]);

	expect(stderr, `unexpected stderr: ${stderr}`).toBe('');
	expect(stdout).toContain('TS2322');
	expect(stdout).not.toContain('Extended diagnostics');
	expect(stdout).not.toContain('Memory report');
});

test('extended_diagnostics_renders_stderr_block_and_keeps_stdout_identical', async () => {
	const { project } = fixtureProject();
	const plain = await runCli(['--project', project]);
	const extended = await runCli(['--project', project, '--extendedDiagnostics']);

	expect(plain.stderr).toBe('');
	expect(extended.stdout, 'stdout bytes must not change').toBe(plain.stdout);

	expect(extended.stderr).toContain('Extended diagnostics:');
	for (const label of [
		'files:',
		'source files:',
		'dependency declaration files:',
		'default lib files:',
		'diagnostics:',
		'jobs:',
		'allocator:',
		'checking:',
		'total:',
		'peak physical footprint:',
		'finish physical footprint:',
		'peak rss:',
	]) {
		expect(extended.stderr, `missing ${JSON.stringify(label)}`).toContain(label);
	}
	expect(extended.stderr).toContain('diagnostics:');
	expect(extended.stderr).toContain(' 1');
});

test('memory_report_renders_stderr_block_and_keeps_stdout_identical', async () => {
	const { project } = fixtureProject();
	const plain = await runCli(['--project', project]);
	const memory = await runCli(['--project', project, '--memoryReport']);

	expect(memory.stdout, 'stdout bytes must not change').toBe(plain.stdout);
	expect(memory.stderr).toContain('Memory report:');
	expect(memory.stderr).toContain('peak physical footprint:');
	expect(memory.stderr).toContain('finish physical footprint:');
	expect(memory.stderr).toContain('peak rss:');
	expect(memory.stderr).not.toContain('Extended diagnostics');
});

test('report_json_writes_versioned_report_and_keeps_stdout_identical', async () => {
	const { root, project } = fixtureProject();
	const reportPath = join(root, 'report.json');

	const plain = await runCli(['--project', project]);
	const withReport = await runCli(['--project', project, '--reportJson', reportPath]);

	expect(withReport.stdout, 'stdout bytes must not change').toBe(plain.stdout);

	const parsed = readJson(reportPath);
	expect(parsed.schemaVersion).toBe(1);

	const summary = parsed.summary;
	for (const key of [
		'files',
		'sourceFiles',
		'dependencyDeclarationFiles',
		'defaultLibFiles',
		'diagnostics',
		'wallTimeMs',
		'jobs',
		'allocator',
	]) {
		expect(summary, `summary missing ${key}`).toHaveProperty(key);
	}
	const phases = parsed.phases;
	for (const key of [
		'configProjectLoadingMs',
		'fileDiscoveryMs',
		'defaultLibLoadingMs',
		'packageDeclarationDiscoveryMs',
		'importGraphExpansionMs',
		'pathMappingResolutionMs',
		'checkingMs',
		'diagnosticRenderingMs',
		'totalMs',
	]) {
		expect(phases, `phases missing ${key}`).toHaveProperty(key);
		expect(typeof phases[key], `${key} must be a number`).toBe('number');
	}
	const memory = parsed.memory;
	for (const key of ['peakPhysicalBytes', 'finishPhysicalBytes', 'peakRssBytes']) {
		expect(memory, `memory missing ${key}`).toHaveProperty(key);
		const value = memory[key];
		expect(
			value === null || (Number.isInteger(value) && value > 0),
			`${key} must be null or a positive byte count, got ${value}`,
		).toBe(true);
	}

	expect(summary.diagnostics).toBe(1);
	expect(Number.isInteger(summary.files)).toBe(true);
	expect(summary.files).toBe(
		summary.sourceFiles + summary.dependencyDeclarationFiles + summary.defaultLibFiles,
	);
	expect(summary.sourceFiles).toBeGreaterThanOrEqual(1);
	expect(summary.wallTimeMs).toBeGreaterThan(0);
});

function keySequence(value: unknown, out: string[]): void {
	if (Array.isArray(value)) {
		for (const item of value) keySequence(item, out);
	} else if (value !== null && typeof value === 'object') {
		for (const [key, child] of Object.entries(value)) {
			out.push(key);
			keySequence(child, out);
		}
	}
}

function zeroNumbers(value: unknown): unknown {
	if (typeof value === 'number') return 0;
	if (Array.isArray(value)) return value.map(zeroNumbers);
	if (value !== null && typeof value === 'object') {
		return Object.fromEntries(
			Object.entries(value).map(([key, child]) => [key, zeroNumbers(child)]),
		);
	}
	return value;
}

test('report_json_is_deterministic_across_runs_modulo_volatile_values', async () => {
	const { root, project } = fixtureProject();
	const firstPath = join(root, 'report-a.json');
	const secondPath = join(root, 'report-b.json');

	for (const path of [firstPath, secondPath]) {
		await runCli(['--project', project, '--reportJson', path]);
	}

	const first = readJson(firstPath);
	const second = readJson(secondPath);

	const firstKeys: string[] = [];
	const secondKeys: string[] = [];
	keySequence(first, firstKeys);
	keySequence(second, secondKeys);
	expect(secondKeys, 'key sequence must be identical').toEqual(firstKeys);

	expect(
		JSON.stringify(zeroNumbers(second)),
		'reports must be byte-identical once volatile numeric values are zeroed',
	).toBe(JSON.stringify(zeroNumbers(first)));
});

test('report_json_jobs_metadata_reflects_jobs_flag', async () => {
	const { root, project } = fixtureProject();

	const autoPath = join(root, 'report-auto.json');
	await runCli(['--project', project, '--reportJson', autoPath]);
	expect(readJson(autoPath).summary.jobs).toBe('auto');

	const serialPath = join(root, 'report-serial.json');
	await runCli(['--project', project, '--jobs', '1', '--reportJson', serialPath]);
	expect(readJson(serialPath).summary.jobs).toBe(1);
});

test('report_json_counts_dependency_declarations', async () => {
	const root = tempProject({
		'tsconfig.json': TSCONFIG,
		'node_modules/dep/package.json': '{ "name": "dep", "types": "index.d.ts" }',
		'node_modules/dep/index.d.ts': 'export declare const answer: number;\n',
		'src/index.ts': 'import { answer } from "dep";\nlet value: string = answer;\n',
	});
	const reportPath = join(root, 'report.json');
	await runCli(['--project', join(root, 'tsconfig.json'), '--reportJson', reportPath]);

	const parsed = readJson(reportPath);
	expect(parsed.summary.dependencyDeclarationFiles).toBe(1);
	expect(parsed.summary.sourceFiles).toBe(1);
});

test('report_flags_compose_with_format_json', async () => {
	const { root, project } = fixtureProject();
	const reportPath = join(root, 'report.json');

	const { stdout, stderr } = await runCli([
		'--project',
		project,
		'--format',
		'json',
		'--extendedDiagnostics',
		'--reportJson',
		reportPath,
	]);

	expect(JSON.parse(stdout).diagnostics).toHaveLength(1);
	expect(stderr).toContain('Extended diagnostics:');
	expect(existsSync(reportPath)).toBe(true);
});

test('report_flags_require_project', async () => {
	const root = tempProject({ 'index.ts': 'let value: string = 123;' });
	const file = join(root, 'index.ts');

	for (const [flag, extra] of [
		['--extendedDiagnostics', null],
		['--memoryReport', null],
		['--reportJson', 'out.json'],
	] as const) {
		const args: string[] = ['--ignoreConfig', flag];
		if (extra !== null) args.push(extra);
		args.push(file);
		const output = await runCliRaw(args);
		expect(output.exitCode, `${flag} without --project must fail`).not.toBe(0);
		expect(output.stderr, `unexpected stderr for ${flag}`).toContain(`${flag} requires --project`);
		expect(output.stdout).toBe('');
	}
});

test('report_flags_conflict_with_show_config', async () => {
	const { root, project } = fixtureProject();
	const reportPath = join(root, 'report.json');

	const output = await runCliRaw([
		'--project',
		project,
		'--showConfig',
		'--reportJson',
		reportPath,
	]);
	expect(output.exitCode).not.toBe(0);
	expect(output.stderr).toContain('--reportJson cannot be used with --showConfig');
	expect(existsSync(reportPath)).toBe(false);
});

test('report_json_write_failure_is_a_clear_error', async () => {
	const { root, project } = fixtureProject();
	const missingDir = join(root, 'missing-dir', 'report.json');

	const output = await runCliRaw(['--project', project, '--reportJson', missingDir]);
	expect(output.exitCode).toBe(1);
	expect(output.stderr).toContain('failed to write');
});
