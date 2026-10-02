// @filename: src/index.ts
import * as pkg from "pkg-ns"; let missing = pkg.missing;
// @filename: types/pkg-ns.d.ts
declare module "pkg-ns" { export const value: string; export function getName(): string; }
