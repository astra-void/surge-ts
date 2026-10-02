import { expect, test } from 'vitest';

import { surge, tempProject, tsc } from '../harness/cli.ts';

const TSCONFIG =
	'{ "compilerOptions": { "noEmit": true, "strict": true }, "include": ["src/**/*.ts"] }';

const ANSI = /\u001b\[[0-9;]*m/g;

// cwd is the project root so file labels are emitted relative to the project
// (`src/index.ts`), matching how `tsc` is run.
async function runIn(root: string, args: string[], forceColor: boolean): Promise<string> {
	const result = await surge(args, {
		cwd: root,
		env: { NO_COLOR: undefined, FORCE_COLOR: forceColor ? '1' : undefined },
	});
	expect(result.exitCode === 0 || result.stdout !== '', result.stderr).toBe(true);
	return result.stdout;
}

async function tscOutput(root: string, args: string[]): Promise<string> {
	return (await tsc(args, { cwd: root })).stdout;
}

function constProject(): string {
	return tempProject({
		'tsconfig.json': TSCONFIG,
		'src/index.ts': 'const a = 1;\nexport {};\na = 3;\n',
	});
}

test('default_non_pretty_matches_tsc_one_line', async () => {
	const root = constProject();
	const args = ['--project', 'tsconfig.json', '--pretty', 'false'];
	const stdout = await runIn(root, args, false);
	expect(stdout).toBe(await tscOutput(root, args));
});

test('default_style_is_tsc_not_custom', async () => {
	const root = constProject();
	// No style flag at all: the default must be tsc-compatible, not the custom
	// `error[TS....]` / ` --> ` Rust-style output.
	const stdout = await runIn(root, ['--project', 'tsconfig.json', '--pretty', 'false'], false);
	expect(stdout).toContain('(3,1): error TS2588:');
	expect(stdout).not.toContain('error[TS2588]');
	expect(stdout).not.toContain(' --> ');
});

// tsc colours `--pretty true` output unconditionally; surge colours only when
// asked, so the uncoloured frame is tsc's with the escapes removed.
test('pretty_true_no_color_matches_tsc_frame', async () => {
	const root = constProject();
	const args = ['--project', 'tsconfig.json', '--pretty', 'true'];
	const stdout = await runIn(root, args, false);
	expect(stdout).toBe((await tscOutput(root, args)).replace(ANSI, ''));
});

test('pretty_true_color_matches_tsc_ansi', async () => {
	const root = constProject();
	const args = ['--project', 'tsconfig.json', '--pretty', 'true'];
	const stdout = await runIn(root, args, true);
	expect(stdout).toBe(await tscOutput(root, args));
});

test('no_color_env_disables_ansi_even_when_pretty', async () => {
	const root = constProject();
	const result = await surge(['--project', 'tsconfig.json', '--pretty', 'true'], {
		cwd: root,
		env: { FORCE_COLOR: '1', NO_COLOR: '1' },
	});
	expect(result.stdout).not.toContain('\u001b');
	expect(result.stdout).toContain('src/index.ts:3:1 - error TS2588:');
});

test('pretty_false_equals_default_when_piped', async () => {
	const root = constProject();
	const explicit = await runIn(root, ['--project', 'tsconfig.json', '--pretty', 'false'], false);
	// Piped stdout (not a TTY) defaults to non-pretty, matching `--pretty false`.
	const defaulted = await runIn(root, ['--project', 'tsconfig.json'], false);
	expect(defaulted).toBe(explicit);
});

test('multiple_diagnostics_in_one_file', async () => {
	const root = tempProject({
		'tsconfig.json': TSCONFIG,
		'src/index.ts': 'export {};\nlet a: number = "x";\nlet b: number = "y";\n',
	});
	const plainArgs = ['--project', 'tsconfig.json', '--pretty', 'false'];
	const stdout = await runIn(root, plainArgs, false);
	const lines = stdout.split('\n').filter((line) => line !== '');
	expect(lines, `expected two diagnostics, got: ${stdout}`).toHaveLength(2);
	expect(lines[0].startsWith('src/index.ts(2,')).toBe(true);
	expect(lines[0]).toContain('error TS2322:');
	expect(lines[1].startsWith('src/index.ts(3,')).toBe(true);
	expect(stdout).toBe(await tscOutput(root, plainArgs));

	const prettyArgs = ['--project', 'tsconfig.json', '--pretty', 'true'];
	const pretty = await runIn(root, prettyArgs, false);
	expect(pretty).toContain('Found 2 errors in the same file, starting at: src/index.ts:2');
	expect(pretty).toBe((await tscOutput(root, prettyArgs)).replace(ANSI, ''));
});

test('multiple_files_footer_and_ordering', async () => {
	const root = tempProject({
		'tsconfig.json': TSCONFIG,
		'src/a.ts': 'export {};\nlet a: number = "x";\n',
		'src/b.ts': 'export {};\nlet b: number = "y";\n',
	});

	const plainArgs = ['--project', 'tsconfig.json', '--pretty', 'false'];
	const stdout = await runIn(root, plainArgs, false);
	const aIndex = stdout.indexOf('src/a.ts');
	const bIndex = stdout.indexOf('src/b.ts');
	expect(aIndex, 'a.ts present').toBeGreaterThanOrEqual(0);
	expect(bIndex, 'b.ts present').toBeGreaterThanOrEqual(0);
	expect(aIndex, 'a.ts should be emitted before b.ts').toBeLessThan(bIndex);
	expect(stdout).toContain('error TS2322:');
	expect(stdout).toBe(await tscOutput(root, plainArgs));

	const prettyArgs = ['--project', 'tsconfig.json', '--pretty', 'true'];
	const pretty = await runIn(root, prettyArgs, false);
	expect(pretty).toContain('Found 2 errors in 2 files.');
	expect(pretty).toContain('Errors  Files');
	expect(pretty).toContain('1  src/a.ts:2');
	expect(pretty).toContain('1  src/b.ts:2');
	expect(pretty).toBe((await tscOutput(root, prettyArgs)).replace(ANSI, ''));
});

test('custom_style_is_preserved_behind_flag', async () => {
	const root = constProject();
	const stdout = await runIn(
		root,
		['--project', 'tsconfig.json', '--diagnostic-style', 'custom'],
		false,
	);
	expect(stdout).toContain('error[TS2588]');
	expect(stdout).toContain(' --> ');
});

test('json_style_emits_machine_readable', async () => {
	const root = constProject();
	const stdout = await runIn(root, ['--project', 'tsconfig.json', '--diagnostic-style', 'json'], false);
	expect(JSON.parse(stdout).diagnostics[0].code).toBe('TS2588');
});

test('camel_case_style_alias_also_works', async () => {
	const root = constProject();
	const stdout = await runIn(
		root,
		['--project', 'tsconfig.json', '--diagnosticStyle', 'custom'],
		false,
	);
	expect(stdout).toContain('error[TS2588]');
});

test('success_prints_nothing', async () => {
	const root = tempProject({
		'tsconfig.json': TSCONFIG,
		'src/index.ts': 'export {};\nlet ok: string = "ok";\n',
	});
	const args = ['--project', 'tsconfig.json', '--pretty', 'true'];
	const stdout = await runIn(root, args, false);
	expect(stdout, `expected empty output, got: ${stdout}`).toBe('');
	expect(stdout).toBe(await tscOutput(root, args));
});
