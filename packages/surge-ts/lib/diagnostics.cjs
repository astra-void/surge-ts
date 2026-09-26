"use strict";

// Diagnostic formatting, as TypeScript's `formatDiagnostic`,
// `formatDiagnostics` and `formatDiagnosticsWithColorAndContext` write it.
const path = require("node:path");
const { DiagnosticCategory } = require("./enums.cjs");

const GUTTER_STYLE = "\u001b[7m";
const GUTTER_SEPARATOR = " ";
const RESET = "\u001b[0m";
const ELLIPSIS = "...";
const COLORS = { grey: "\u001b[90m", red: "\u001b[91m", yellow: "\u001b[93m", blue: "\u001b[94m", cyan: "\u001b[96m" };

function diagnosticCategoryName(diagnostic, lowerCase = true) {
	const name = DiagnosticCategory[diagnostic.category];
	return lowerCase ? name.toLowerCase() : name;
}

function flattenDiagnosticMessageText(diagnostic, newLine, indent = 0) {
	if (typeof diagnostic === "string") return diagnostic;
	if (diagnostic === undefined) return "";
	let result = "";
	if (indent) {
		result += newLine;
		for (let index = 0; index < indent; index++) result += "  ";
	}
	result += diagnostic.messageText;
	indent++;
	for (const next of diagnostic.next ?? []) result += flattenDiagnosticMessageText(next, newLine, indent);
	return result;
}

function relativeFileName(fileName, host) {
	const directory = host.getCurrentDirectory();
	if (!path.isAbsolute(fileName) || !directory) return fileName;
	const canonical = (name) => (host.getCanonicalFileName ? host.getCanonicalFileName(name) : name);
	const from = canonical(directory).replace(/\\/g, "/").replace(/\/$/, "").split("/");
	const to = fileName.replace(/\\/g, "/").split("/");
	const toCanonical = canonical(fileName).replace(/\\/g, "/").split("/");
	let common = 0;
	while (common < from.length && common < toCanonical.length && from[common] === toCanonical[common]) common++;
	if (common === 0) return fileName;
	return [...from.slice(common).map(() => ".."), ...to.slice(common)].join("/");
}

function formatDiagnostic(diagnostic, host) {
	const newLine = host.getNewLine();
	const message = `${diagnosticCategoryName(diagnostic)} TS${diagnostic.code}: ${flattenDiagnosticMessageText(diagnostic.messageText, newLine)}${newLine}`;
	if (diagnostic.file) {
		const { line, character } = diagnostic.file.getLineAndCharacterOfPosition(diagnostic.start);
		return `${relativeFileName(diagnostic.file.fileName, host)}(${line + 1},${character + 1}): ${message}`;
	}
	return message;
}

function formatDiagnostics(diagnostics, host) {
	let output = "";
	for (const diagnostic of diagnostics) output += formatDiagnostic(diagnostic, host);
	return output;
}

function categoryColor(category) {
	switch (category) {
		case DiagnosticCategory.Error:
			return COLORS.red;
		case DiagnosticCategory.Warning:
			return COLORS.yellow;
		case DiagnosticCategory.Message:
			return COLORS.blue;
		default:
			return COLORS.grey;
	}
}

function colored(text, color) {
	return color + text + RESET;
}

function formatCodeSpan(file, start, length, indent, squiggleColor, host) {
	const newLine = host.getNewLine();
	const { line: firstLine, character: firstLineChar } = file.getLineAndCharacterOfPosition(start);
	const { line: lastLine, character: lastLineChar } = file.getLineAndCharacterOfPosition(start + length);
	const lastLineInFile = file.getLineAndCharacterOfPosition(file.text.length).line;
	const hasMoreThanFiveLines = lastLine - firstLine >= 4;
	let gutterWidth = `${lastLine + 1}`.length;
	if (hasMoreThanFiveLines) gutterWidth = Math.max(ELLIPSIS.length, gutterWidth);
	let context = "";
	for (let line = firstLine; line <= lastLine; line++) {
		context += newLine;
		if (hasMoreThanFiveLines && firstLine + 1 < line && line < lastLine - 1) {
			context += indent + colored(ELLIPSIS.padStart(gutterWidth), GUTTER_STYLE) + GUTTER_SEPARATOR + newLine;
			line = lastLine - 1;
		}
		const lineStart = file.getPositionOfLineAndCharacter(line, 0);
		const lineEnd = line < lastLineInFile ? file.getPositionOfLineAndCharacter(line + 1, 0) : file.text.length;
		const lineContent = file.text.slice(lineStart, lineEnd).trimEnd().replace(/\t/g, " ");
		context += indent + colored(`${line + 1}`.padStart(gutterWidth), GUTTER_STYLE) + GUTTER_SEPARATOR;
		context += lineContent + newLine;
		context += indent + colored("".padStart(gutterWidth), GUTTER_STYLE) + GUTTER_SEPARATOR;
		context += squiggleColor;
		if (line === firstLine) {
			const lastCharForLine = line === lastLine ? lastLineChar : undefined;
			context += lineContent.slice(0, firstLineChar).replace(/\S/g, " ");
			context += lineContent.slice(firstLineChar, lastCharForLine).replace(/./g, "~");
		} else if (line === lastLine) {
			context += lineContent.slice(0, lastLineChar).replace(/./g, "~");
		} else {
			context += lineContent.replace(/./g, "~");
		}
		context += RESET;
	}
	return context;
}

function formatLocation(file, start, host) {
	const { line, character } = file.getLineAndCharacterOfPosition(start);
	return `${colored(relativeFileName(file.fileName, host), COLORS.cyan)}:${colored(`${line + 1}`, COLORS.yellow)}:${colored(`${character + 1}`, COLORS.yellow)}`;
}

function formatDiagnosticsWithColorAndContext(diagnostics, host) {
	const newLine = host.getNewLine();
	let output = "";
	for (const diagnostic of diagnostics) {
		if (diagnostic.file) output += `${formatLocation(diagnostic.file, diagnostic.start, host)} - `;
		output += colored(diagnosticCategoryName(diagnostic), categoryColor(diagnostic.category));
		output += colored(` TS${diagnostic.code}: `, COLORS.grey);
		output += flattenDiagnosticMessageText(diagnostic.messageText, newLine);
		if (diagnostic.file) {
			output += newLine;
			output += formatCodeSpan(diagnostic.file, diagnostic.start, diagnostic.length, "", categoryColor(diagnostic.category), host);
		}
		output += newLine;
	}
	return output;
}

/** tsc's `sortAndDeduplicateDiagnostics`. */
function sortAndDeduplicateDiagnostics(diagnostics) {
	const key = (diagnostic) => [diagnostic.file?.fileName ?? "", diagnostic.start ?? -1, diagnostic.length ?? -1, diagnostic.code];
	const sorted = [...diagnostics].sort((left, right) => {
		const a = key(left);
		const b = key(right);
		for (let index = 0; index < a.length; index++) {
			if (a[index] < b[index]) return -1;
			if (a[index] > b[index]) return 1;
		}
		const leftText = flattenDiagnosticMessageText(left.messageText, "\n");
		const rightText = flattenDiagnosticMessageText(right.messageText, "\n");
		return leftText < rightText ? -1 : leftText > rightText ? 1 : 0;
	});
	return sorted.filter((diagnostic, index) => {
		if (index === 0) return true;
		const previous = sorted[index - 1];
		return !(
			previous.file === diagnostic.file &&
			previous.start === diagnostic.start &&
			previous.length === diagnostic.length &&
			previous.code === diagnostic.code &&
			flattenDiagnosticMessageText(previous.messageText, "\n") === flattenDiagnosticMessageText(diagnostic.messageText, "\n")
		);
	});
}

module.exports = {
	diagnosticCategoryName,
	flattenDiagnosticMessageText,
	formatDiagnostic,
	formatDiagnostics,
	formatDiagnosticsWithColorAndContext,
	sortAndDeduplicateDiagnostics,
};
