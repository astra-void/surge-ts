"use strict";

// JSDoc for `getDocumentationComment` / `getJsDocTags`, read from the
// `/** ... */` comments in a declaration's leading trivia. surge's parser
// keeps no JSDoc tree, so this is a text reading: the comment's prose, and
// each `@tag` with the text after it; inline `{@link ...}` stays as written.
const { SyntaxKind } = require("./enums.cjs");

/** The node a declaration's JSDoc is written on: a variable's statement. */
function commentHost(declaration) {
	if (declaration.kind === SyntaxKind.VariableDeclaration) {
		const list = declaration.parent;
		const statement = list?.parent;
		if (list?.kind === SyntaxKind.VariableDeclarationList && statement?.kind === SyntaxKind.VariableStatement) {
			return statement;
		}
	}
	return declaration;
}

/** The `/** ... *\/` comments in the host's leading trivia, as their inner text. */
function jsDocBlocks(declaration) {
	const host = commentHost(declaration);
	const trivia = host.getSourceFile().text.substring(host.pos, host.getStart());
	const blocks = [];
	const pattern = /\/\*\*(?!\/)([\s\S]*?)\*\//g;
	for (let match = pattern.exec(trivia); match !== null; match = pattern.exec(trivia)) {
		blocks.push(
			match[1]
				.split(/\r?\n/)
				.map((line) => line.replace(/^\s*\*?\s?/, ""))
				.join("\n"),
		);
	}
	return blocks;
}

function splitTags(block) {
	const lines = block.split("\n");
	const prose = [];
	const tags = [];
	for (const line of lines) {
		const tag = /^@(\S+)\s?(.*)$/.exec(line.trim());
		if (tag) tags.push({ name: tag[1], text: tag[2] });
		else if (tags.length) tags[tags.length - 1].text += `\n${line}`;
		else prose.push(line);
	}
	return { prose: prose.join("\n").trim(), tags };
}

function documentationOf(declarations) {
	const parts = [];
	for (const declaration of declarations) {
		for (const block of jsDocBlocks(declaration)) {
			const { prose } = splitTags(block);
			if (!prose) continue;
			if (parts.length) parts.push({ text: "\n", kind: "lineBreak" });
			parts.push({ text: prose, kind: "text" });
		}
	}
	return parts;
}

function jsDocTagsOf(declarations) {
	const tags = [];
	for (const declaration of declarations) {
		for (const block of jsDocBlocks(declaration)) {
			for (const tag of splitTags(block).tags) {
				const text = tag.text.trim();
				tags.push(text ? { name: tag.name, text: [{ text, kind: "text" }] } : { name: tag.name });
			}
		}
	}
	return tags;
}

module.exports = { documentationOf, jsDocTagsOf };
