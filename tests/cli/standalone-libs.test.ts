// The released binary must check a project with no TypeScript installation
// anywhere: no `typescript` package, no `node_modules`, no `package.json`, and
// no surge source tree. These tests run the real CLI in a scratch directory and
// assert the bundled standard library is what answers.

import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';

import { expect, test } from 'vitest';

import { expectSameDiagnosticsAsTsc, surge, tempProject } from '../harness/cli.ts';
import type { ProcessResult } from '../harness/run.ts';

const ES2022_TSCONFIG =
	'{ "compilerOptions": { "target": "ES2022", "strict": true, "noEmit": true } }';

// Fails if any ancestor of `dir` could supply TypeScript declarations, which
// would make a passing result meaningless.
function expectNoTypescriptInstallation(dir: string): void {
	let current = dir;
	for (;;) {
		for (const name of ['node_modules', 'package.json', 'typescript']) {
			expect(
				existsSync(join(current, name)),
				`${name} exists at ${current}; the standalone fixture is not isolated`,
			).toBe(false);
		}
		const parent = dirname(current);
		if (parent === current) return;
		current = parent;
	}
}

function standaloneProject(tsconfig: string, source: string): string {
	const root = tempProject({ 'tsconfig.json': tsconfig, 'index.ts': source });
	expectNoTypescriptInstallation(root);
	return root;
}

function check(root: string, extraArgs: string[] = []): Promise<ProcessResult> {
	return surge(['--project', join(root, 'tsconfig.json'), ...extraArgs], { cwd: root });
}

function describeRun(run: ProcessResult): string {
	return `stdout: ${run.stdout}\nstderr: ${run.stderr}`;
}

test('checks_builtin_declarations_with_no_typescript_installed', async () => {
	const root = standaloneProject(
		ES2022_TSCONFIG,
		`const xs: string[] = ["hello"];

const p: Promise<number> = Promise.resolve(123);

const m = new Map<string, number>();
m.set("foo", 1);

async function test(): Promise<number> {
  return await p;
}

const keep = [xs, m, test];
`,
	);

	const run = await check(root);
	expect(run.exitCode, describeRun(run)).toBe(0);
	for (const complaint of ['could not find', 'lib*.d.ts', 'node_modules', 'falling back']) {
		expect(run.stderr, `stderr mentions ${JSON.stringify(complaint)}`).not.toContain(complaint);
	}
	await expectSameDiagnosticsAsTsc(root, ['--project', 'tsconfig.json']);
});

// The embedded declarations must actually participate in checking, not merely
// silence missing-global errors.
test('embedded_declarations_are_type_checked', async () => {
	const root = standaloneProject(ES2022_TSCONFIG, 'const xs: string[] = [];\n\nxs.push(123);\n');

	const run = await check(root);
	expect(run.exitCode, describeRun(run)).toBe(2);
	await expectSameDiagnosticsAsTsc(root, ['--project', 'tsconfig.json'], { messages: true });
});

test('utility_types_and_collections_resolve_from_the_snapshot', async () => {
	const root = standaloneProject(
		ES2022_TSCONFIG,
		`const set = new Set<number>();
const partial: Partial<{ a: string }> = {};
type R = Record<string, number>;
const record: R = {};

async function foo() {
  const value = await Promise.resolve(1);
  return value;
}

const keep = [set, partial, record, foo];
`,
	);

	const run = await check(root);
	expect(run.exitCode, describeRun(run)).toBe(0);
	await expectSameDiagnosticsAsTsc(root, ['--project', 'tsconfig.json']);
});

test('dom_lib_is_not_loaded_unless_selected', async () => {
	const source = `const el: HTMLElement | null = document.querySelector("#app");
fetch("/api");
const controller = new AbortController();
const keep = [el, controller];
`;

	const withoutDom = standaloneProject(
		'{ "compilerOptions": { "target": "ES2022", "lib": ["ES2022"], "strict": true, "noEmit": true } }',
		source,
	);
	const withoutRun = await check(withoutDom);
	expect(withoutRun.exitCode, `expected DOM globals to be absent: ${withoutRun.stdout}`).toBe(2);
	await expectSameDiagnosticsAsTsc(withoutDom, ['--project', 'tsconfig.json'], { messages: true });

	const withDom = standaloneProject(
		'{ "compilerOptions": { "target": "ES2022", "lib": ["ES2022", "DOM"], "strict": true, "noEmit": true } }',
		source,
	);
	const withRun = await check(withDom);
	expect(withRun.exitCode, describeRun(withRun)).toBe(0);
	await expectSameDiagnosticsAsTsc(withDom, ['--project', 'tsconfig.json']);
});

// `lib` names are matched the way TypeScript matches them: case-insensitively,
// with or without the `lib.`/`.d.ts` affixes.
test('lib_names_are_normalized_like_typescript', async () => {
	for (const lib of ['"ES2022", "DOM.Iterable", "DOM"', '"es2022", "dom"']) {
		const root = standaloneProject(
			`{ "compilerOptions": { "target": "ES2022", "lib": [${lib}], "strict": true, "noEmit": true } }`,
			'const el = document.body;\nconst keep = [el];\n',
		);
		const run = await check(root);
		expect(run.exitCode, `lib ${lib}: ${describeRun(run)}`).toBe(0);
		await expectSameDiagnosticsAsTsc(root, ['--project', 'tsconfig.json']);
	}
});

test('no_lib_disables_the_bundled_snapshot', async () => {
	const root = standaloneProject(
		'{ "compilerOptions": { "target": "ES2022", "noLib": true, "strict": true, "noEmit": true } }',
		'const xs: string[] = [];\nconst keep = xs;\n',
	);

	const run = await check(root);
	expect(run.exitCode, `expected noLib to remove the global types: ${run.stdout}`).toBe(2);
	await expectSameDiagnosticsAsTsc(root, ['--project', 'tsconfig.json'], { messages: true });
});

// `/// <reference lib="..." />` between bundled files must be followed, so a
// single seed pulls in its whole transitive graph.
test('reference_lib_graph_resolves_within_the_snapshot', async () => {
	// `es2022` only references its predecessors; everything used below is
	// declared in a file reachable solely through that chain.
	const root = standaloneProject(
		'{ "compilerOptions": { "target": "ES2022", "lib": ["ES2022"], "strict": true, "noEmit": true } }',
		`const map = new Map<string, number>();
const entries = [...map.entries()];
const flat = [[1], [2]].flat();
const trimmed = "  x  ".trimStart();
const big = 2n ** 3n;
const keep = [entries, flat, trimmed, big];
`,
	);

	const run = await check(root);
	expect(run.exitCode, describeRun(run)).toBe(0);
	await expectSameDiagnosticsAsTsc(root, ['--project', 'tsconfig.json']);
});

// An override that cannot be honoured must say so and fall back to the bundled
// snapshot rather than silently checking against nothing.
test('missing_lib_path_override_warns_and_falls_back', async () => {
	const root = standaloneProject(ES2022_TSCONFIG, 'const xs: string[] = [];\nconst keep = xs;\n');

	const run = await check(root, ['--typescript-lib-path', join(root, 'no-such-lib-dir')]);
	expect(run.exitCode, describeRun(run)).toBe(0);
	expect(run.stderr, 'expected a fallback warning naming the override').toContain(
		'--typescript-lib-path',
	);
	expect(run.stderr).toContain('bundled TypeScript');
});

// Requesting the project's installed TypeScript when there is none must warn
// and still check successfully against the bundled snapshot.
test('physical_libs_request_without_typescript_warns_and_falls_back', async () => {
	const root = standaloneProject(ES2022_TSCONFIG, 'const xs: string[] = [];\nconst keep = xs;\n');

	const run = await check(root, ['--physicalLibs']);
	expect(run.exitCode, describeRun(run)).toBe(0);
	expect(run.stderr, 'expected a fallback warning').toContain('--physicalLibs');
	expect(run.stderr).toContain('bundled TypeScript');
});
