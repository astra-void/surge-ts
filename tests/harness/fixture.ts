import { readFileSync } from 'node:fs';
import { basename, relative } from 'node:path';

import {
	canonicalCompilerOption,
	isHarnessOnlyDirective,
	parseCompilerOptionValue,
} from './compiler-options.ts';
import { WORKSPACE_ROOT } from './paths.ts';

export type VirtualFile = { name: string; text: string };

export type CompareField = 'messages' | 'order' | 'spans';

export type Fixture = {
	name: string;
	/** Workspace-relative path of the fixture source. */
	path: string;
	files: VirtualFile[];
	compilerOptions: Record<string, unknown>;
	surgeArgs: string[];
	/**
	 * Codes surge must report, in order, for behaviour tsc has no counterpart
	 * for (the native diagnostic profile, surge-only flags). When set, tsc is
	 * not consulted.
	 */
	expectedCodes: string[] | null;
	compare: Set<CompareField>;
	skip: string | null;
};

/**
 * The checker defaults the Rust suites were written against: the strict family
 * minus the flags `CheckerOptions::default()` turns off. strictFunctionTypes and
 * alwaysStrict have no surge switch, so they stay on through `strict`. A fixture
 * that sets `@strict` itself gets plain tsc semantics for the whole family.
 */
export const BASE_COMPILER_OPTIONS: Readonly<Record<string, unknown>> = {
	strict: true,
	noImplicitAny: false,
	noImplicitThis: false,
	strictBindCallApply: false,
	strictBuiltinIteratorReturn: false,
	strictPropertyInitialization: false,
	useUnknownInCatchVariables: false,
};

const DIRECTIVE = /^\s*\/\/\s*@([A-Za-z][\w-]*)\s*:\s*(.*?)\s*$/;

export function loadFixture(
	absolutePath: string,
	overrides: { name?: string; singleFileName?: string } = {},
): Fixture {
	const text = readFileSync(absolutePath, 'utf8');
	const path = relative(WORKSPACE_ROOT, absolutePath);
	return parseFixture(text, {
		name: overrides.name ?? basename(absolutePath).replace(/\.[^.]+$/, ''),
		path,
		singleFileName: overrides.singleFileName ?? basename(absolutePath),
	});
}

export function parseFixture(
	text: string,
	meta: { name: string; path: string; singleFileName: string },
): Fixture {
	const directiveOptions: Record<string, unknown> = {};
	const surgeArgs: string[] = [];
	const compare = new Set<CompareField>();
	let expectedCodes: string[] | null = null;
	let skip: string | null = null;

	const files: VirtualFile[] = [];
	let current: { name: string; lines: string[] } = {
		name: meta.singleFileName,
		lines: [],
	};
	let sawFilename = false;

	const lines = text.split('\n');
	for (const line of lines) {
		const directive = DIRECTIVE.exec(line);
		if (directive === null) {
			current.lines.push(line);
			continue;
		}
		const [, rawName, value] = directive;
		const name = rawName.toLowerCase();

		if (name === 'filename') {
			flush(files, current, sawFilename);
			current = { name: value, lines: [] };
			sawFilename = true;
			continue;
		}

		// Compiler-option directives stay in the source, as in the TypeScript
		// harness, so line numbers match the fixture file.
		current.lines.push(line);

		switch (name) {
			case 'surge-args':
				surgeArgs.push(...value.split(/\s+/).filter(Boolean));
				continue;
			case 'surge-expect':
				expectedCodes =
					value === 'none' ? [] : value.split(/[\s,]+/).filter(Boolean);
				continue;
			case 'surge-compare':
				for (const field of value.split(/[\s,]+/).filter(Boolean)) {
					if (field !== 'messages' && field !== 'order' && field !== 'spans') {
						throw new Error(`${meta.path}: unknown @surge-compare field ${field}`);
					}
					compare.add(field);
				}
				continue;
			case 'surge-skip':
				skip = value || 'skipped';
				continue;
		}

		if (isHarnessOnlyDirective(name)) continue;
		const option = canonicalCompilerOption(name);
		if (option === undefined) {
			throw new Error(`${meta.path}: unknown fixture directive @${rawName}`);
		}
		directiveOptions[option] = parseCompilerOptionValue(option, value);
	}
	flush(files, current, sawFilename);

	if (files.length === 0) {
		throw new Error(`${meta.path}: fixture has no source files`);
	}

	const compilerOptions =
		'strict' in directiveOptions
			? directiveOptions
			: { ...BASE_COMPILER_OPTIONS, ...directiveOptions };

	return {
		name: meta.name,
		path: meta.path,
		files,
		compilerOptions,
		surgeArgs,
		expectedCodes,
		compare,
		skip,
	};
}

function flush(
	files: VirtualFile[],
	current: { name: string; lines: string[] },
	sawFilename: boolean,
): void {
	const text = current.lines.join('\n');
	// Before the first `@filename` marker only directives are expected; any real
	// code there becomes a file named after the fixture, as upstream does.
	if (!sawFilename && current.lines.every((line) => isBlankOrDirective(line))) {
		return;
	}
	files.push({ name: current.name, text });
}

function isBlankOrDirective(line: string): boolean {
	return line.trim() === '' || DIRECTIVE.test(line);
}
