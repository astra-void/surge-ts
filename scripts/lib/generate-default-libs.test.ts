import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import { generateDefaultLibs } from "./generate-default-libs.ts";

const workspaceRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const typescriptLibDir = path.join(workspaceRoot, "node_modules/typescript/lib");
const generatedLibDir = path.join(workspaceRoot, "crates/surge-ts-checker/generated-libs");

test("generateDefaultLibs copies the local TypeScript lib files deterministically", () => {
  const first = generateDefaultLibs();
  const second = generateDefaultLibs();

  assert.equal(first.stableHash, second.stableHash);
  assert.ok(first.generatedFiles.includes("lib.es5.d.ts"));
  assert.ok(first.generatedFiles.includes("lib.es2024.full.d.ts"));
  assert.ok(first.generatedFiles.includes("lib.dom.d.ts"));
  assert.ok(!first.generatedFiles.includes("lib.es.generated.d.ts"));
  assert.ok(!first.generatedFiles.includes("lib.dom.generated.d.ts"));

  const sourceDom = fs.readFileSync(path.join(typescriptLibDir, "lib.dom.d.ts"), "utf8");
  const copiedDom = fs.readFileSync(path.join(generatedLibDir, "lib.dom.d.ts"), "utf8");
  assert.equal(copiedDom, sourceDom);

  const sourceEs5 = fs.readFileSync(path.join(typescriptLibDir, "lib.es5.d.ts"), "utf8");
  const copiedEs5 = fs.readFileSync(path.join(generatedLibDir, "lib.es5.d.ts"), "utf8");
  assert.equal(copiedEs5, sourceEs5);

  const manifest = JSON.parse(
    fs.readFileSync(path.join(generatedLibDir, "manifest.json"), "utf8"),
  ) as typeof first;
  assert.deepEqual(manifest, first);
});
