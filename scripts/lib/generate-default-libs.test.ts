import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "vitest";

import { generateDefaultLibs, resolveTypeScriptLibDir } from "./generate-default-libs.ts";

const workspaceRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const typescriptLibDir = resolveTypeScriptLibDir();
const generatedLibDir = path.join(workspaceRoot, "crates/surge-ts-checker/generated-libs");

test("generateDefaultLibs copies the local TypeScript lib files deterministically", () => {
  const first = generateDefaultLibs();
  const second = generateDefaultLibs();

  expect(first.stableHash).toBe(second.stableHash);
  expect(first.generatedFiles).toContain("lib.es5.d.ts");
  expect(first.generatedFiles).toContain("lib.es2024.full.d.ts");
  expect(first.generatedFiles).toContain("lib.dom.d.ts");
  expect(first.generatedFiles).not.toContain("lib.es.generated.d.ts");
  expect(first.generatedFiles).not.toContain("lib.dom.generated.d.ts");

  const sourceDom = fs.readFileSync(path.join(typescriptLibDir, "lib.dom.d.ts"), "utf8");
  const copiedDom = fs.readFileSync(path.join(generatedLibDir, "lib.dom.d.ts"), "utf8");
  expect(copiedDom).toBe(sourceDom);

  const sourceEs5 = fs.readFileSync(path.join(typescriptLibDir, "lib.es5.d.ts"), "utf8");
  const copiedEs5 = fs.readFileSync(path.join(generatedLibDir, "lib.es5.d.ts"), "utf8");
  expect(copiedEs5).toBe(sourceEs5);

  const manifest = JSON.parse(
    fs.readFileSync(path.join(generatedLibDir, "manifest.json"), "utf8"),
  ) as typeof first;
  expect(manifest).toStrictEqual(first);
});
