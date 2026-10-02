// @filename: src/index.ts
import { missing } from "pkg"; let x: number = missing;
// @filename: types/pkg.d.ts
declare module "pkg" { export const value: string; }
