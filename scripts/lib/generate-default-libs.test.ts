import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import { extractAuthenticatorTransport, generateDefaultLibs } from "./generate-default-libs.ts";

const workspaceRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const domLibPath = path.join(workspaceRoot, "node_modules/typescript/lib/lib.dom.d.ts");

test("extractAuthenticatorTransport reads the local DOM lib", () => {
  assert.ok(fs.existsSync(domLibPath), `missing ${domLibPath}`);
  const source = fs.readFileSync(domLibPath, "utf8");
  assert.deepEqual(extractAuthenticatorTransport(source), [
    "ble",
    "cable",
    "hybrid",
    "internal",
    "nfc",
    "smart-card",
    "usb",
  ]);
});

test("generateDefaultLibs writes deterministic generated files with the supported surface", () => {
  const first = generateDefaultLibs();
  const second = generateDefaultLibs();

  assert.equal(first.stableHash, second.stableHash);
  assert.deepEqual(first.generatedFiles, [
    "lib.es.generated.d.ts",
    "lib.dom.generated.d.ts",
  ]);

  const esPath = path.join(
    workspaceRoot,
    "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
  );
  const domPath = path.join(
    workspaceRoot,
    "crates/typescript-rust-checker/generated-libs/lib.dom.generated.d.ts",
  );

  const esSource = fs.readFileSync(esPath, "utf8");
  const domSource = fs.readFileSync(domPath, "utf8");

  for (const needle of [
    "interface Array<T> {",
    "map<U>(callback: (value: T, index: number, array: T[]) => U): U[];",
    "find(callback: (value: T, index: number, array: T[]) => unknown): T | undefined;",
    "join(separator?: string): string;",
    "includes(value: T): boolean;",
    "push(...items: T[]): number;",
    "interface Promise<T> {}",
    "interface Map<K, V> {",
    "get(key: K): any;",
    "set(key: K, value: V): any;",
    "has(key: K): boolean;",
    "delete(key: K): boolean;",
    "clear(): void;",
    "size: number;",
    "interface Uint8Array extends Array<number> {}",
    "declare function Uint8Array(value?: unknown): Uint8Array;",
  ]) {
    assert.ok(esSource.includes(needle), `missing ${needle} in lib.es.generated.d.ts`);
  }

  assert.ok(
    domSource.includes("type AuthenticatorTransport =\n  | \"ble\"") &&
      domSource.includes("  | \"usb\"\n;"),
    "missing AuthenticatorTransport union in lib.dom.generated.d.ts",
  );
  assert.ok(domSource.includes("interface TextEncoder {"));
  assert.ok(domSource.includes("declare function TextEncoder(): TextEncoder;"));
});
