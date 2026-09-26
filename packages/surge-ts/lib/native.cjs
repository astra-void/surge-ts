"use strict";

// Loads the native binding: an explicit SURGE_TS_NATIVE path, the binary
// built beside the package, or the per-platform package napi-rs publishes.
const path = require("node:path");
const { existsSync } = require("node:fs");

function load() {
	const override = process.env.SURGE_TS_NATIVE;
	if (override) return require(path.resolve(override));
	const local = path.join(__dirname, "..", `surge-ts.${process.platform}-${process.arch}.node`);
	if (existsSync(local)) return require(local);
	try {
		return require(`@surge-ts/binding-${process.platform}-${process.arch}`);
	} catch (error) {
		throw new Error(
			`surge-ts: no native binding for ${process.platform}-${process.arch}. ` +
				"Build one with `node scripts/build-native.cjs` in packages/surge-ts, " +
				"or point SURGE_TS_NATIVE at a built binding.",
			{ cause: error },
		);
	}
}

module.exports = load();
