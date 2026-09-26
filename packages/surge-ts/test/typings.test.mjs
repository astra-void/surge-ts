// The package's type declarations (index.d.ts) against its runtime.
import { fileURLToPath } from "node:url";
import typescript from "typescript-6";
import { describe, expect, test } from "vitest";
import surge from "../index.mjs";

describe("typings", () => {
	test("declare no value the runtime lacks", () => {
		const declarations = fileURLToPath(new URL("../index.d.ts", import.meta.url));
		const program = typescript.createProgram([declarations], { noEmit: true, types: [] });
		const checker = program.getTypeChecker();
		const module = checker.getSymbolAtLocation(program.getSourceFile(declarations));
		const values = checker
			.getExportsOfModule(module)
			.filter((symbol) => {
				const target = symbol.flags & typescript.SymbolFlags.Alias ? checker.getAliasedSymbol(symbol) : symbol;
				return (target.flags & typescript.SymbolFlags.Value) !== 0;
			})
			.map((symbol) => symbol.name);
		expect(values.filter((name) => surge[name] === undefined)).toEqual([]);
		expect(values).toContain("createProgram");
		expect(values).not.toContain("transpileModule");
	});

	test("type-check a TypeScript user of the package", () => {
		const consumer = fileURLToPath(new URL("./types/consumer.ts", import.meta.url));
		const program = typescript.createProgram([consumer], {
			strict: true,
			noEmit: true,
			module: typescript.ModuleKind.NodeNext,
			moduleResolution: typescript.ModuleResolutionKind.NodeNext,
			types: [],
		});
		const messages = typescript
			.getPreEmitDiagnostics(program)
			.map((diagnostic) => typescript.flattenDiagnosticMessageText(diagnostic.messageText, "\n"));
		expect(messages).toEqual([]);
	});
});
