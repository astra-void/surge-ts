// A TypeScript user of the package, type-checked by test/typings.test.mjs.
import ts from "surge-ts";
import { createProgram, SyntaxKind } from "surge-ts";

const program: ts.Program = createProgram(["index.ts"], { strict: true, noEmit: true });
const checker: ts.TypeChecker = program.getTypeChecker();

export function names(sourceFile: ts.SourceFile): string[] {
	const found: string[] = [];
	ts.forEachChild(sourceFile, function visit(node: ts.Node): void {
		if (ts.isIdentifier(node) && node.kind === SyntaxKind.Identifier) {
			const symbol: ts.Symbol | undefined = checker.getSymbolAtLocation(node);
			if (symbol) found.push(`${symbol.getName()}: ${checker.typeToString(checker.getTypeOfSymbolAtLocation(symbol, node))}`);
		}
		ts.forEachChild(node, visit);
	});
	return found;
}

export const diagnostics: readonly ts.Diagnostic[] = ts.getPreEmitDiagnostics(program);

// @ts-expect-error surge-ts does not provide TypeScript's transpiler.
ts.transpileModule("", {});
