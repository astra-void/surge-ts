import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

import { describe, expect, test } from 'vitest';

import { formatDiagnostic } from './diagnostics.ts';
import { type Fixture, loadFixture } from './fixture.ts';
import { WORKSPACE_ROOT } from './paths.ts';
import {
	type CheckerRun,
	crashDescription,
	materialize,
	removeProject,
	runSurge,
	runTsc,
	tsconfigFor,
} from './run.ts';

const FIXTURE = /\.(?:[cm]?[jt]sx?)$/;

/**
 * Every fixture source under `dir`, in a stable order: tracked files plus new
 * ones .gitignore does not exclude. Asking git keeps out the .js emit a
 * compiler run without --noEmit leaves beside the sources, which .gitignore
 * already names as build output.
 */
export function fixturePaths(dir: string): string[] {
	const listed = execFileSync(
		'git',
		['ls-files', '-z', '--cached', '--others', '--exclude-standard', '--', '.'],
		{ cwd: dir, encoding: 'utf8' },
	);
	return [...new Set(listed.split('\0'))]
		.filter((path) => FIXTURE.test(path) && existsSync(join(dir, path)))
		.sort()
		.map((path) => join(dir, path));
}

/** Registers one concurrent test per fixture file under `dir`. */
export function fixtureSuite(dir: string): void {
	for (const path of fixturePaths(dir)) {
		const fixture = loadFixture(path);
		fixtureTest(relative(dir, path), fixture);
	}
}

/** Registers a `describe` per immediate subdirectory of `dir`. */
export function fixtureSuites(dir: string): void {
	for (const entry of readdirSync(dir).sort()) {
		const path = join(dir, entry);
		if (statSync(path).isDirectory()) {
			describe(entry, () => fixtureSuite(path));
		}
	}
}

export function fixtureTest(title: string, fixture: Fixture): void {
	if (fixture.skip !== null) {
		test.skip(`${title} (${fixture.skip})`, () => {});
		return;
	}
	test.concurrent(title, async () => {
		await checkFixture(fixture);
	});
}

export async function checkFixture(fixture: Fixture): Promise<void> {
	const fields = {
		messages: fixture.compare.has('messages'),
		spans: fixture.compare.has('spans'),
	};
	const dir = materialize(fixture);
	try {
		if (fixture.expectedCodes !== null) {
			const surge = await runSurge(fixture, dir, false);
			assertNoCrash(fixture, surge);
			expect(
				surge.diagnostics.map((diagnostic) => diagnostic.code),
				report(fixture, surge, null),
			).toEqual(fixture.expectedCodes);
			return;
		}

		const [surge, tsc] = await Promise.all([
			runSurge(fixture, dir, fields.spans),
			runTsc(fixture, dir, fields.spans),
		]);
		assertNoCrash(fixture, surge);
		const ordered = fixture.compare.has('order');
		const render = (run: CheckerRun) => {
			const lines = run.diagnostics.map((diagnostic) => formatDiagnostic(diagnostic, fields));
			return ordered ? lines : lines.sort();
		};
		expect(render(surge), report(fixture, surge, tsc)).toEqual(render(tsc));
	} finally {
		removeProject(dir);
	}
}

function assertNoCrash(fixture: Fixture, surge: CheckerRun): void {
	const crash = crashDescription(surge);
	if (crash !== null) {
		throw new Error(`surge ${crash} on ${fixture.path}\n${surge.stderr}`);
	}
}

function report(fixture: Fixture, surge: CheckerRun, tsc: CheckerRun | null): string {
	const sections = [
		`fixture ${fixture.path}`,
		`tsconfig.json:\n${tsconfigFor(fixture)}`,
		`surge output:\n${surge.stdout}${surge.stderr}`,
	];
	if (tsc !== null) sections.push(`tsc output:\n${tsc.stdout}${tsc.stderr}`);
	return sections.join('\n');
}

export function workspacePath(...segments: string[]): string {
	return join(WORKSPACE_ROOT, ...segments);
}
