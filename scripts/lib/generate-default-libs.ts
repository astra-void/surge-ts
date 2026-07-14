import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const GENERATOR_VERSION = "v0.87-copy-native-typescript-libs";
const WORKSPACE_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const TYPESCRIPT_PACKAGE_JSON = path.join(WORKSPACE_ROOT, "node_modules/typescript/package.json");
const OUTPUT_DIR = path.join(
  WORKSPACE_ROOT,
  "crates/surge-ts-checker/generated-libs",
);

// TypeScript 7.0 (the native compiler) ships the `lib.*.d.ts` files inside the
// platform-specific binary package (@typescript/typescript-<platform>-<arch>),
// not in `node_modules/typescript/lib` as the 6.x JS package did. Resolve the
// installed platform package's lib directory; the workspace pins pnpm, so cover
// both a hoisted layout and pnpm's `.pnpm` virtual store.
export const platformLibPackage = `@typescript/typescript-${process.platform}-${process.arch}`;

export function resolveTypeScriptLibDir(): string {
  const platformPkg = platformLibPackage;
  const pnpmDirName = platformPkg.replace("/", "+");
  const candidates = [
    path.join(WORKSPACE_ROOT, "node_modules", platformPkg, "lib"),
    ...fs
      .globSync(
        path.join(
          WORKSPACE_ROOT,
          "node_modules/.pnpm",
          `${pnpmDirName}@*`,
          "node_modules",
          platformPkg,
          "lib",
        ),
      )
      .sort((left, right) => right.localeCompare(left)),
  ];

  for (const candidate of candidates) {
    if (fs.existsSync(path.join(candidate, "lib.es5.d.ts"))) {
      return candidate;
    }
  }

  throw new Error(
    `could not locate TypeScript 7.0 native lib directory for ${platformPkg}. ` +
      `Install workspace dependencies (pnpm install) so the platform package is present.`,
  );
}

const TYPESCRIPT_LIB_DIR = resolveTypeScriptLibDir();

type Manifest = {
  generatorVersion: string;
  generatedFrom: {
    typescriptPackageVersion: string;
    sourceLibDir: string;
  };
  generatedFiles: string[];
  stableHash: string;
};

type CopiedLib = {
  fileName: string;
  contents: string;
};

export function generateDefaultLibs(): Manifest {
  const typescriptPackageVersion = readJson<{ version?: string }>(TYPESCRIPT_PACKAGE_JSON).version;
  if (!typescriptPackageVersion) {
    throw new Error(`missing version in ${TYPESCRIPT_PACKAGE_JSON}`);
  }

  const libs = readTypeScriptLibs();
  if (!libs.some((lib) => lib.fileName === "lib.es5.d.ts")) {
    throw new Error(`missing lib.es5.d.ts in ${TYPESCRIPT_LIB_DIR}`);
  }
  if (!libs.some((lib) => lib.fileName === "lib.dom.d.ts")) {
    throw new Error(`missing lib.dom.d.ts in ${TYPESCRIPT_LIB_DIR}`);
  }

  fs.rmSync(OUTPUT_DIR, { recursive: true, force: true });
  fs.mkdirSync(OUTPUT_DIR, { recursive: true });

  for (const lib of libs) {
    fs.writeFileSync(path.join(OUTPUT_DIR, lib.fileName), lib.contents);
  }

  const manifest: Manifest = {
    generatorVersion: GENERATOR_VERSION,
    generatedFrom: {
      typescriptPackageVersion,
      sourceLibDir: `node_modules/${platformLibPackage}/lib`,
    },
    generatedFiles: libs.map((lib) => lib.fileName),
    stableHash: stableHash(
      GENERATOR_VERSION,
      typescriptPackageVersion,
      ...libs.flatMap((lib) => [lib.fileName, lib.contents]),
    ),
  };

  fs.writeFileSync(
    path.join(OUTPUT_DIR, "manifest.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );

  return manifest;
}

function readTypeScriptLibs(): CopiedLib[] {
  if (!fs.existsSync(TYPESCRIPT_LIB_DIR)) {
    throw new Error(
      `missing TypeScript lib directory: ${TYPESCRIPT_LIB_DIR}. Install the local workspace dependencies first.`,
    );
  }

  return fs
    .readdirSync(TYPESCRIPT_LIB_DIR)
    .filter((fileName) => /^lib\..*\.d\.ts$/.test(fileName))
    .sort((left, right) => left.localeCompare(right))
    .map((fileName) => ({
      fileName,
      contents: fs.readFileSync(path.join(TYPESCRIPT_LIB_DIR, fileName), "utf8"),
    }));
}

function readRequired(fileName: string): string {
  if (!fs.existsSync(fileName)) {
    throw new Error(
      `missing TypeScript package file: ${fileName}. Install the local workspace dependencies first.`,
    );
  }

  return fs.readFileSync(fileName, "utf8");
}

function readJson<T>(fileName: string): T {
  return JSON.parse(readRequired(fileName)) as T;
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
      `copied ${manifest.generatedFiles.length} TypeScript lib files`,
      `typescript ${manifest.generatedFrom.typescriptPackageVersion}`,
      `hash ${manifest.stableHash}`,
    ].join("\n"),
  );
}

const directExecution =
  process.argv[1] !== undefined &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (directExecution) {
  main();
}
