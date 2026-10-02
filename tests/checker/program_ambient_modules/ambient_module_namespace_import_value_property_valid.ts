// @filename: src/index.ts
import * as pkg from "pkg-ns"; let ok: string = pkg.value; let name: string = pkg.getName();
// @filename: types/pkg-ns.d.ts
declare module "pkg-ns" { export const value: string; export function getName(): string; export interface User { name: string; } }
