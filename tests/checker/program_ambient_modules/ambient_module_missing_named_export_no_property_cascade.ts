// @filename: src/index.ts
import { missing } from "pkg"; missing.property;
// @filename: types/pkg.d.ts
declare module "pkg" { export const value: string; }
