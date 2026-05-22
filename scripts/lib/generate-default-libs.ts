import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import * as ts from "typescript";

const GENERATOR_VERSION = "v0.85";
const WORKSPACE_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const TYPESCRIPT_PACKAGE_JSON = path.join(WORKSPACE_ROOT, "node_modules/typescript/package.json");
const TYPESCRIPT_LIB_DIR = path.join(WORKSPACE_ROOT, "node_modules/typescript/lib");
const OUTPUT_DIR = path.join(
  WORKSPACE_ROOT,
  "crates/typescript-rust-checker/generated-libs",
);
const ES_LIB_SOURCE = path.join(TYPESCRIPT_LIB_DIR, "lib.es5.d.ts");
const ES_COLLECTION_LIB_SOURCE = path.join(TYPESCRIPT_LIB_DIR, "lib.es2015.collection.d.ts");
const ES_PROMISE_LIB_SOURCE = path.join(TYPESCRIPT_LIB_DIR, "lib.es2015.promise.d.ts");
const ES_TYPED_ARRAYS_LIB_SOURCE = path.join(TYPESCRIPT_LIB_DIR, "lib.es2017.typedarrays.d.ts");
const DOM_LIB_SOURCE = path.join(TYPESCRIPT_LIB_DIR, "lib.dom.d.ts");
const CANONICAL_AUTHENTICATOR_TRANSPORT_VALUES = [
  "ble",
  "cable",
  "hybrid",
  "internal",
  "nfc",
  "smart-card",
  "usb",
] as const;

type Manifest = {
  generatorVersion: string;
  generatedFrom: {
    typescriptPackageVersion: string;
    sourceLibFiles: string[];
  };
  generatedFiles: string[];
  generatedDeclarations: string[];
  stableHash: string;
};

type GeneratedFile = {
  fileName: string;
  contents: string;
};

export function generateDefaultLibs(): Manifest {
  const typescriptPackageVersion = readJson<{ version?: string }>(TYPESCRIPT_PACKAGE_JSON).version;
  if (!typescriptPackageVersion) {
    throw new Error(`missing version in ${TYPESCRIPT_PACKAGE_JSON}`);
  }

  const esSource = readRequired(ES_LIB_SOURCE);
  const esCollectionSource = readRequired(ES_COLLECTION_LIB_SOURCE);
  const esPromiseSource = readRequired(ES_PROMISE_LIB_SOURCE);
  const esTypedArraysSource = readRequired(ES_TYPED_ARRAYS_LIB_SOURCE);
  const domSource = readRequired(DOM_LIB_SOURCE);
  validateSourcePresence(esSource, [
    "Array",
    "ReadonlyArray",
    "String",
    "Number",
    "Boolean",
    "Date",
    "Math",
    "JSON",
    "Object",
    "decodeURIComponent",
    "isNaN",
    "Partial",
    "Pick",
    "Parameters",
    "Record",
    "Omit",
    "ReturnType",
  ], ES_LIB_SOURCE);
  validateSourcePresence(esCollectionSource, ["Map"], ES_COLLECTION_LIB_SOURCE);
  validateSourcePresence(esPromiseSource, ["Promise", "PromiseLike"], ES_PROMISE_LIB_SOURCE);
  validateSourcePresence(esTypedArraysSource, ["Uint8Array"], ES_TYPED_ARRAYS_LIB_SOURCE);
  validateSourcePresence(domSource, [
    "TextEncoder",
    "AuthenticatorTransport",
    "Crypto",
    "Headers",
    "Request",
    "Response",
    "URL",
    "console",
    "fetch",
    "globalThis",
  ], DOM_LIB_SOURCE);

  const authTransport = extractAuthenticatorTransport(domSource);
  const files = buildGeneratedFiles(authTransport);
  fs.mkdirSync(OUTPUT_DIR, { recursive: true });

  for (const file of files) {
    writeFileIfChanged(path.join(OUTPUT_DIR, file.fileName), file.contents);
  }

  const manifest: Manifest = {
    generatorVersion: GENERATOR_VERSION,
    generatedFrom: {
      typescriptPackageVersion,
      sourceLibFiles: [
        "node_modules/typescript/lib/lib.es5.d.ts",
        "node_modules/typescript/lib/lib.es2015.collection.d.ts",
        "node_modules/typescript/lib/lib.es2015.promise.d.ts",
        "node_modules/typescript/lib/lib.es2017.typedarrays.d.ts",
        "node_modules/typescript/lib/lib.dom.d.ts",
      ],
    },
    generatedFiles: files.map((file) => file.fileName),
    generatedDeclarations: [
      "Array",
      "ArrayConstructor",
      "AuthenticatorTransport",
      "Boolean",
      "Crypto",
      "Date",
      "Headers",
      "JSON",
      "Map",
      "Math",
      "Number",
      "Object",
      "Promise",
      "PromiseConstructor",
      "PromiseLike",
      "Partial",
      "Pick",
      "Parameters",
      "Record",
      "Omit",
      "ReturnType",
      "ReadonlyArray",
      "Request",
      "Response",
      "console",
      "String",
      "TextEncoder",
      "URL",
      "Uint8Array",
      "fetch",
      "globalThis",
      "decodeURIComponent",
      "isNaN",
    ],
    stableHash: stableHash(
      GENERATOR_VERSION,
      typescriptPackageVersion,
      esSource,
      esCollectionSource,
      esPromiseSource,
      esTypedArraysSource,
      domSource,
      files.map((file) => file.contents).join("\n"),
    ),
  };

  writeFileIfChanged(
    path.join(OUTPUT_DIR, "manifest.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );

  return manifest;
}

function buildGeneratedFiles(authenticatorTransportValues: string[]): GeneratedFile[] {
  return [
    {
      fileName: "lib.es.generated.d.ts",
      contents: buildEsLib(),
    },
    {
      fileName: "lib.dom.generated.d.ts",
      contents: buildDomLib(authenticatorTransportValues),
    },
  ];
}

function buildEsLib(): string {
  return [
    "// Generated from the local TypeScript lib sources. Do not edit by hand.",
    "",
    "interface Array<T> {",
    "  length: number;",
    "  map<U>(callback: (value: T, index: number, array: T[]) => U): U[];",
    "  find(callback: (value: T, index: number, array: T[]) => unknown): T | undefined;",
    "  join(separator?: string): string;",
    "  includes(value: T): boolean;",
    "  push(...items: T[]): number;",
    "}",
    "",
    "interface ReadonlyArray<T> {",
    "  length: number;",
    "}",
    "",
    "interface ArrayConstructor {",
    "  from(value: unknown): any[];",
    "}",
    "",
    "declare const Array: ArrayConstructor;",
    "",
    "interface Promise<T> {}",
    "",
    "interface PromiseLike<T> {}",
    "",
    "interface PromiseConstructor {",
    "  resolve<T>(value: T): Promise<T>;",
    "  all<T>(values: Promise<T>[]): Promise<T[]>;",
    "}",
    "",
    "declare const Promise: PromiseConstructor;",
    "",
    "interface Map<K, V> {",
    "  get(key: K): any;",
    "  set(key: K, value: V): any;",
    "  has(key: K): boolean;",
    "  delete(key: K): boolean;",
    "  clear(): void;",
    "  size: number;",
    "}",
    "",
    "interface Uint8Array extends Array<number> {}",
    "",
    "type Date = any;",
    "",
    "interface String {",
    "  replace(searchValue: string | RegExp, replaceValue: string): string;",
    "  split(separator: string | RegExp): string[];",
    "  slice(start?: number, end?: number): string;",
    "  toLowerCase(): string;",
    "  toUpperCase(): string;",
    "  padStart(maxLength: number, fillString?: string): string;",
    "  charCodeAt(index: number): number;",
    "}",
    "",
    "interface Number {",
    "  toString(radix?: number): string;",
    "}",
    "",
    "interface Boolean {}",
    "",
    "interface ObjectConstructor {",
    "  keys(value: unknown): string[];",
    "}",
    "",
    "declare const Object: ObjectConstructor;",
    "",
    "declare const Date: {",
    "  now: () => number;",
    "};",
    "",
    "declare const Math: {",
    "  floor: (value: number) => number;",
    "  max: (a: number, b?: number, c?: number, d?: number) => number;",
    "  min: (a: number, b?: number, c?: number, d?: number) => number;",
    "  round: (value: number) => number;",
    "};",
    "",
    "declare const JSON: {",
    "  stringify: (value: unknown) => string;",
    "  parse: (value: string) => unknown;",
    "};",
    "",
    "declare function decodeURIComponent(encodedURIComponent: string): string;",
    "",
    "declare function isNaN(value: unknown): boolean;",
    "",
    "declare function Number(value?: unknown): number;",
    "declare function String(value?: unknown): string;",
    "declare function Boolean(value?: unknown): boolean;",
    "declare function Map<K, V>(): Map<K, V>;",
    "declare function Uint8Array(value?: unknown): Uint8Array;",
    "",
    "/**",
    " * Make all properties in T optional",
    " */",
    "type Partial<T> = {",
    "  [P in keyof T]?: T[P];",
    "};",
    "",
    "/**",
    " * From T, pick a set of properties whose keys are in the union K",
    " */",
    "type Pick<T, K extends keyof T> = {",
    "  [P in K]: T[P];",
    "};",
    "",
    "type Record<K extends keyof any, T> = { [P in K]: T };",
    "",
    "/**",
    " * Construct a type with the properties of T except for those in type K.",
    " */",
    "type Omit<T, K extends keyof any> = Pick<T, Exclude<keyof T, K>>;",
    "",
    "type Parameters<T> = unknown[];",
    "",
    "type ReturnType<T> = unknown;",
    "",
  ].join("\n");
}

function buildDomLib(authenticatorTransportValues: string[]): string {
  const transportUnion = authenticatorTransportValues.map((value) => `  | ${JSON.stringify(value)}`);

  return [
    "// Generated from the local TypeScript lib sources. Do not edit by hand.",
    "",
    "interface TextEncoder {",
    "  encode(input?: string): Uint8Array;",
    "}",
    "",
    "declare function TextEncoder(): TextEncoder;",
    "",
    "type AuthenticatorTransport =",
    ...transportUnion,
    ";",
    "",
    "interface Crypto {",
    "  getRandomValues(array: Uint8Array): Uint8Array;",
    "}",
    "",
    "interface Headers {}",
    "",
    "interface Request {}",
    "",
    "interface Response {",
    "  ok: boolean;",
    "  status: number;",
    "  json(): unknown;",
    "}",
    "",
    "interface URL {}",
    "",
    "interface Console {",
    "  log: any;",
    "  warn: any;",
    "  error: any;",
    "}",
    "",
    "declare function fetch(input: unknown, init?: unknown): Promise<Response>;",
    "",
    "declare function Headers(init?: unknown): Headers;",
    "declare function Request(input?: unknown, init?: unknown): Request;",
    "declare function Response(body?: unknown, init?: unknown): Response;",
    "declare function URL(url: string): URL;",
    "declare const crypto: Crypto;",
    "declare const console: Console;",
    "",
    "declare const globalThis: {",
    "  crypto: Crypto;",
    "};",
    "",
  ].join("\n");
}

export function extractAuthenticatorTransport(source: string): string[] {
  const sourceFile = ts.createSourceFile("lib.dom.d.ts", source, ts.ScriptTarget.Latest, true);

  for (const statement of sourceFile.statements) {
    if (!ts.isTypeAliasDeclaration(statement) || statement.name.text !== "AuthenticatorTransport") {
      continue;
    }

    if (!ts.isUnionTypeNode(statement.type)) {
      throw new Error("AuthenticatorTransport is not a union type");
    }

    const values: string[] = [];
    for (const member of statement.type.types) {
      if (!ts.isLiteralTypeNode(member) || !ts.isStringLiteral(member.literal)) {
        throw new Error("AuthenticatorTransport contains a non-string literal member");
      }

      values.push(member.literal.text);
    }

    if (values.length === 0) {
      throw new Error("AuthenticatorTransport union is empty");
    }

    return normalizeAuthenticatorTransportValues(values);
  }

  throw new Error("failed to locate AuthenticatorTransport in lib.dom.d.ts");
}

function normalizeAuthenticatorTransportValues(values: string[]): string[] {
  const canonical = new Set(CANONICAL_AUTHENTICATOR_TRANSPORT_VALUES);
  const normalized = [...CANONICAL_AUTHENTICATOR_TRANSPORT_VALUES];
  const extras = [...new Set(values.filter((value) => !canonical.has(value)))].sort((left, right) =>
    left.localeCompare(right),
  );

  normalized.push(...extras);
  return normalized;
}

function validateSourcePresence(source: string, names: string[], fileName: string): void {
  for (const name of names) {
    const pattern = new RegExp(`\\b${escapeRegExp(name)}\\b`);
    if (!pattern.test(source)) {
      throw new Error(`missing ${name} in ${fileName}`);
    }
  }
}

function readRequired(fileName: string): string {
  if (!fs.existsSync(fileName)) {
    throw new Error(
      `missing TypeScript lib file: ${fileName}. Install the local workspace dependencies first.`,
    );
  }

  return fs.readFileSync(fileName, "utf8");
}

function readJson<T>(fileName: string): T {
  return JSON.parse(readRequired(fileName)) as T;
}

function writeFileIfChanged(fileName: string, contents: string): void {
  if (fs.existsSync(fileName)) {
    const current = fs.readFileSync(fileName, "utf8");
    if (current === contents) {
      return;
    }
  }

  fs.writeFileSync(fileName, contents);
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function stableHash(...parts: string[]): string {
  const hash = crypto.createHash("sha256");
  for (const part of parts) {
    hash.update(part);
    hash.update("\0");
  }
  return hash.digest("hex");
}

function main(): void {
  const manifest = generateDefaultLibs();
  console.log(
    [
      `generated ${manifest.generatedFiles.length} default-lib files`,
      `typescript ${manifest.generatedFrom.typescriptPackageVersion}`,
      `hash ${manifest.stableHash}`,
    ].join("\n"),
  );
}

const directExecution =
  process.argv[1] !== undefined &&
  path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url));

if (directExecution) {
  main();
}
