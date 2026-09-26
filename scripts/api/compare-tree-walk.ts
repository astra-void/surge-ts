// Compares the tsc-syntax crate's tree walk (its `tree_walk` example, which
// visits children as `properties::child_properties` describes them) with
// TypeScript 6's `forEachChild` walk of the same files: one
// `<Kind> <start> <end>` line per node, pre-order.
//
// Usage: tsx scripts/api/compare-tree-walk.ts [--bin <tree_walk>] [--node-modules] [--show <n>] [--list] <file|dir>...
//
// Files TypeScript reports syntax errors for are skipped (error recovery is
// not what this checks), and so are files with non-ASCII text: the crate
// reports UTF-8 byte offsets, TypeScript UTF-16 ones.

import { spawnSync } from "node:child_process";
import { readdirSync, readFileSync, realpathSync, statSync } from "node:fs";
import path from "node:path";
import ts from "typescript-6";

const SOURCE_EXTENSIONS = [".ts", ".tsx", ".mts", ".cts"];
const CHUNK_SIZE = 200;

// `ts.SyntaxKind[kind]` answers with whichever enum member was declared last
// for a value, so the range markers win (`FirstStatement` for
// VariableStatement, `AssertClause` for ImportAttributes, ...). The first
// declared name is the kind's own.
const kindNames = new Map<number, string>();
for (const [name, value] of Object.entries(ts.SyntaxKind)) {
	if (typeof value === "number" && !kindNames.has(value)) {
		kindNames.set(value, name);
	}
}
// TypeScript's names that typescript-go spells differently.
const TSGO_KIND_NAMES = new Map([
	["EndOfFileToken", "EndOfFile"],
	["JSDocTag", "JSDocUnknownTag"],
]);

function kindName(kind: ts.SyntaxKind): string {
	const name = kindNames.get(kind) ?? String(kind);
	return TSGO_KIND_NAMES.get(name) ?? name;
}

interface Options {
	bin: string | undefined;
	nodeModules: boolean;
	show: number;
	list: boolean;
	paths: string[];
}

function parseArgs(argv: string[]): Options {
	const options: Options = { bin: undefined, nodeModules: false, show: 20, list: false, paths: [] };
	for (let i = 0; i < argv.length; i++) {
		const arg = argv[i];
		if (arg === "--bin") {
			options.bin = argv[++i];
		} else if (arg === "--node-modules") {
			options.nodeModules = true;
		} else if (arg === "--show") {
			options.show = Number(argv[++i]);
		} else if (arg === "--list") {
			options.list = true;
		} else if (arg.startsWith("--")) {
			throw new Error(`unknown option ${arg}`);
		} else {
			options.paths.push(arg);
		}
	}
	if (options.paths.length === 0) {
		throw new Error("usage: compare-tree-walk.ts [--bin <tree_walk>] [--node-modules] [--show <n>] [--list] <file|dir>...");
	}
	return options;
}

function collectFiles(roots: string[], nodeModules: boolean): string[] {
	const files: string[] = [];
	const seen = new Set<string>();
	const visit = (entry: string): void => {
		let real: string;
		try {
			real = realpathSync(entry);
		} catch {
			return;
		}
		if (seen.has(real)) {
			return;
		}
		seen.add(real);
		const stat = statSync(real);
		if (stat.isDirectory()) {
			for (const name of readdirSync(entry).sort()) {
				if (name === ".git" || (name === "node_modules" && !nodeModules)) {
					continue;
				}
				visit(path.join(entry, name));
			}
		} else if (stat.isFile() && SOURCE_EXTENSIONS.some((extension) => entry.endsWith(extension))) {
			files.push(entry);
		}
	};
	for (const root of roots) {
		visit(root);
	}
	return files;
}

interface Walk {
	lines: string[];
	/** Each node's full start, to recognize the `jsx-text-start` deviation. */
	fullStarts: number[];
}

function typescriptWalk(file: string, text: string): Walk | "syntax-errors" {
	const scriptKind = file.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS;
	const sourceFile = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, scriptKind);
	const { parseDiagnostics } = sourceFile as unknown as { parseDiagnostics: readonly ts.Diagnostic[] };
	if (parseDiagnostics.length > 0) {
		return "syntax-errors";
	}
	const walk: Walk = { lines: [], fullStarts: [] };
	const stack: ts.Node[] = [sourceFile];
	while (stack.length > 0) {
		const node = stack.pop()!;
		walk.lines.push(`${kindName(node.kind)} ${node.getStart(sourceFile)} ${node.end}`);
		walk.fullStarts.push(node.pos);
		const children: ts.Node[] = [];
		ts.forEachChild(
			node,
			(child) => {
				children.push(child);
			},
			(nodes) => {
				children.push(...nodes);
			},
		);
		for (let i = children.length - 1; i >= 0; i--) {
			stack.push(children[i]);
		}
	}
	return walk;
}

interface CrateWalks {
	walks: Map<string, string[]>;
	/** Each file's first syntax error, for the files the crate reports one for. */
	syntaxErrors: Map<string, string>;
}

function crateWalks(bin: string, files: string[]): CrateWalks {
	const result = spawnSync(bin, files, { encoding: "utf8", maxBuffer: 1 << 30 });
	if (result.status !== 0) {
		throw new Error(`${bin} failed (${result.status}): ${result.stderr}`);
	}
	const walks = new Map<string, string[]>();
	// A single file's walk has no `== <file>` header.
	let current: string[] = [];
	if (files.length === 1) {
		walks.set(files[0], current);
	}
	for (const line of result.stdout.split("\n")) {
		if (line.startsWith("== ")) {
			current = [];
			walks.set(line.slice(3), current);
		} else if (line !== "") {
			current.push(line);
		}
	}
	const syntaxErrors = new Map<string, string>();
	for (const line of result.stderr.split("\n")) {
		const error = /^(.*?): (TS\d+ at \d+: .*)$/.exec(line);
		if (error && !syntaxErrors.has(error[1])) {
			syntaxErrors.set(error[1], error[2]);
		}
	}
	return { walks, syntaxErrors };
}

// Differences between typescript-go's tree and Strada's, and a known
// deviation outside the property schema, each counted per node.
const KNOWN = {
	"nested-namespace-export":
		"typescript-go gives the body of `namespace A.B` a synthesized zero-width `export` modifier; TypeScript gives it none",
	"array-binding-elision":
		"typescript-go builds a nameless BindingElement for the hole in `[, a]`; TypeScript builds an OmittedExpression",
	"jsx-text-start": "SyntaxTree::start returns a JsxText's full start; tsc's getStart skips its leading whitespace",
} as const;
type Known = keyof typeof KNOWN;

interface Comparison {
	known: Map<Known, number>;
	mismatchAt: number | undefined;
	crateLines: string[];
}

function compare(crateLines: string[], walk: Walk): Comparison {
	const known = new Map<Known, number>();
	const count = (name: Known): void => {
		known.set(name, (known.get(name) ?? 0) + 1);
	};
	const lines: string[] = [];
	for (const line of crateLines) {
		const implicitExport = /^ExportKeyword (\d+) (\d+)$/.exec(line);
		if (implicitExport && implicitExport[1] === implicitExport[2] && lines.at(-1)?.startsWith("ModuleDeclaration ")) {
			count("nested-namespace-export");
			continue;
		}
		lines.push(line);
	}
	let mismatchAt: number | undefined;
	const length = Math.max(lines.length, walk.lines.length);
	for (let i = 0; i < length && mismatchAt === undefined; i++) {
		if (lines[i] === walk.lines[i]) {
			continue;
		}
		const ours = lines[i]?.split(" ");
		const theirs = walk.lines[i]?.split(" ");
		if (
			ours?.[0] === "BindingElement" &&
			theirs?.[0] === "OmittedExpression" &&
			ours[1] === theirs[1] &&
			ours[2] === theirs[2]
		) {
			count("array-binding-elision");
			continue;
		}
		if (
			ours?.[0] === "JsxText" &&
			theirs?.[0] === "JsxText" &&
			ours[2] === theirs[2] &&
			Number(ours[1]) === walk.fullStarts[i]
		) {
			count("jsx-text-start");
			continue;
		}
		mismatchAt = i;
	}
	return { known, mismatchAt, crateLines: lines };
}

function context(lines: string[], at: number): string {
	const from = Math.max(0, at - 4);
	return lines
		.slice(from, at + 4)
		.map((line, index) => `${from + index === at ? ">" : " "} ${from + index}: ${line}`)
		.join("\n");
}

function buildExample(): string {
	const root = path.resolve(import.meta.dirname, "../..");
	const env = { ...process.env, CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS ?? "4" };
	const build = spawnSync("cargo", ["build", "-q", "--example", "tree_walk", "-p", "surge-ts-tsc-syntax"], {
		cwd: root,
		env,
		stdio: "inherit",
	});
	if (build.status !== 0) {
		throw new Error("cargo build --example tree_walk failed");
	}
	return path.join(process.env.CARGO_TARGET_DIR ?? path.join(root, "target"), "debug", "examples", "tree_walk");
}

function main(): void {
	const options = parseArgs(process.argv.slice(2));
	const bin = options.bin ?? buildExample();
	const files = collectFiles(options.paths, options.nodeModules);

	let nonAscii = 0;
	let typescriptSyntaxErrors = 0;
	let identical = 0;
	let identicalModuloKnown = 0;
	let nodes = 0;
	const knownFiles = new Map<Known, number>();
	const knownNodes = new Map<Known, number>();
	const mismatches: string[] = [];
	// Differing files the crate reports a syntax error in and TypeScript 6 does
	// not: the two parsers disagree on the code itself (typescript-go dropped
	// JSDoc type syntax TypeScript 6 still accepts), not on the tree's shape.
	const crateOnlySyntaxErrors: string[] = [];
	let shown = 0;

	for (let offset = 0; offset < files.length; offset += CHUNK_SIZE) {
		const walks = new Map<string, Walk>();
		for (const file of files.slice(offset, offset + CHUNK_SIZE)) {
			const text = readFileSync(file, "utf8");
			if (/[^\x00-\x7f]/.test(text)) {
				nonAscii++;
				continue;
			}
			const walk = typescriptWalk(file, text);
			if (walk === "syntax-errors") {
				typescriptSyntaxErrors++;
				continue;
			}
			walks.set(file, walk);
		}
		if (walks.size === 0) {
			continue;
		}
		const crate = crateWalks(bin, [...walks.keys()]);
		for (const [file, walk] of walks) {
			nodes += walk.lines.length;
			const { known, mismatchAt, crateLines } = compare(crate.walks.get(file) ?? [], walk);
			if (mismatchAt !== undefined) {
				const syntaxError = crate.syntaxErrors.get(file);
				(syntaxError === undefined ? mismatches : crateOnlySyntaxErrors).push(file);
				if (shown++ < options.show) {
					const reason = syntaxError === undefined ? "" : ` (the crate reports ${syntaxError})`;
					console.log(`MISMATCH ${file} at node ${mismatchAt}${reason}`);
					console.log(`  tree_walk:\n${context(crateLines, mismatchAt)}`);
					console.log(`  typescript:\n${context(walk.lines, mismatchAt)}`);
				}
				continue;
			}
			if (known.size === 0) {
				identical++;
				continue;
			}
			identicalModuloKnown++;
			for (const [name, count] of known) {
				knownFiles.set(name, (knownFiles.get(name) ?? 0) + 1);
				knownNodes.set(name, (knownNodes.get(name) ?? 0) + count);
			}
		}
	}

	const compared = identical + identicalModuloKnown + mismatches.length + crateOnlySyntaxErrors.length;
	console.log(`files: ${files.length}`);
	console.log(`  skipped, non-ASCII text: ${nonAscii}`);
	console.log(`  skipped, TypeScript reports syntax errors: ${typescriptSyntaxErrors}`);
	console.log(`  compared: ${compared} (${nodes} nodes in TypeScript's walks)`);
	console.log(`    identical: ${identical}`);
	console.log(`    identical apart from known differences: ${identicalModuloKnown}`);
	for (const name of Object.keys(KNOWN) as Known[]) {
		if (knownFiles.has(name)) {
			console.log(`      ${name}: ${knownFiles.get(name)} files, ${knownNodes.get(name)} nodes (${KNOWN[name]})`);
		}
	}
	console.log(`    differing where only the crate reports a syntax error: ${crateOnlySyntaxErrors.length}`);
	console.log(`    mismatched: ${mismatches.length}`);
	if (options.list) {
		for (const file of [...crateOnlySyntaxErrors, ...mismatches]) {
			console.log(`      ${file}`);
		}
	}
	process.exitCode = mismatches.length > 0 ? 1 : 0;
}

main();
