"use strict";

// TypeScript's enums, rebuilt in declaration order so each has the forward
// and reverse mappings a TypeScript enum object has (`SyntaxKind[80]` is
// "Identifier"). Values are typescript@6.0.3's.
const native = require("./native.cjs");

const enums = {};
for (const [name, entries] of Object.entries(native.enums())) {
	const object = {};
	for (const [member, value] of entries) {
		object[member] = value;
		if (typeof value === "number") object[value] = member;
	}
	enums[name] = object;
}

module.exports = enums;
