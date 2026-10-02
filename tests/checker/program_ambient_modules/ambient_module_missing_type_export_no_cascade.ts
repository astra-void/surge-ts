// @filename: src/index.ts
import type { missing } from "pkg"; type X = missing;
// @filename: types/pkg.d.ts
declare module "pkg" { export const value: string; }
