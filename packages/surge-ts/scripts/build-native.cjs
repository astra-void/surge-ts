// Builds the native binding (crates/surge-ts-node) and places it beside the
// package as `surge-ts.<platform>-<arch>.node`, where lib/native.cjs finds it.
//
// Usage: node scripts/build-native.cjs [--release]
"use strict";

const { execFileSync } = require("node:child_process");
const { copyFileSync, rmSync } = require("node:fs");
const path = require("node:path");

const release = process.argv.includes("--release");
const workspace = path.resolve(__dirname, "..", "..", "..");
const args = ["build", "-p", "surge-ts-node"];
if (release) args.push("--release");
execFileSync("cargo", args, {
	cwd: workspace,
	stdio: "inherit",
	env: { ...process.env, CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS ?? "4" },
});
const library = { darwin: "libsurge_ts_node.dylib", linux: "libsurge_ts_node.so", win32: "surge_ts_node.dll" }[process.platform];
if (!library) throw new Error(`unsupported platform ${process.platform}`);
const built = path.join(workspace, "target", release ? "release" : "debug", library);
const target = path.join(__dirname, "..", `surge-ts.${process.platform}-${process.arch}.node`);
// A new file, not an overwrite: macOS kills a process that loads a Mach-O
// image modified in place after the kernel cached its signature.
rmSync(target, { force: true });
copyFileSync(built, target);
console.log(`wrote ${path.relative(process.cwd(), target)}`);
