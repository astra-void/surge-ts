import { join } from 'node:path';

import { expect } from 'vitest';

import { expectNoCrash, expectSameDiagnosticsAsTsc, surge } from '../harness/cli.ts';
import type { ProcessResult } from '../harness/run.ts';
import { workspacePath } from '../harness/suite.ts';

export type JsonDiagnostic = {
	code: string;
	fileName: string;
	message: string;
	line: number | null;
	column: number | null;
};

export type Json = any;

export function compatProject(name: string): string {
	return workspacePath('tests/compat-projects', name);
}

export function tsconfigPath(root: string): string {
	return join(root, 'tsconfig.json');
}

/**
 * surge mirrors tsc's exit codes: 0 when clean, 2 when diagnostics were
 * reported. Anything else (a panic, a config or usage error) fails the test.
 */
export async function runCli(args: string[], cwd: string): Promise<ProcessResult> {
	const result = await surge(args, { cwd });
	expectNoCrash(result);
	expect([0, 2], `surge exited with ${result.exitCode}\n${result.stderr}`).toContain(
		result.exitCode,
	);
	const json = args.some((arg, index) => arg === '--format' && args[index + 1] === 'json');
	return json ? result : { ...result, stdout: result.stdout.replaceAll('\\', '/') };
}

export async function runCliJson(args: string[], cwd: string): Promise<Json> {
	const { stdout, stderr } = await runCli(args, cwd);
	expect(stderr).toBe('');
	return JSON.parse(stdout);
}

export function diagnosticsOf(parsed: Json): JsonDiagnostic[] {
	expect(Array.isArray(parsed.diagnostics)).toBe(true);
	return parsed.diagnostics;
}

export function codesOf(parsed: Json): string[] {
	return diagnosticsOf(parsed).map((diagnostic) => diagnostic.code);
}

export function linesOf(parsed: Json, code: string): (number | null)[] {
	return diagnosticsOf(parsed)
		.filter((diagnostic) => diagnostic.code === code)
		.map((diagnostic) => diagnostic.line ?? null);
}

export function fingerprintsOf(parsed: Json): string[] {
	return diagnosticsOf(parsed).map(
		(d) => `${d.fileName ?? ''}|${d.code ?? ''}|${d.line ?? 'null'}|${d.column ?? 'null'}|${d.message ?? ''}`,
	);
}

export function expectSameAsTsc(
	cwd: string,
	args: string[] = ['-p', 'tsconfig.json'],
	options: { messages?: boolean } = {},
): Promise<void> {
	return expectSameDiagnosticsAsTsc(cwd, args, options);
}

export function expectSameAsTscOnCompatProject(
	fixture: string,
	options: { messages?: boolean } = {},
): Promise<void> {
	return expectSameAsTsc(compatProject(fixture), ['-p', 'tsconfig.json'], options);
}
