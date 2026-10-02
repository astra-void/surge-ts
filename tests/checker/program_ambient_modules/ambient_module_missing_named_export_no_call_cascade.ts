// @filename: src/index.ts
import { missing } from "pkg"; missing();
// @filename: types/pkg.d.ts
declare module "pkg" { export const value: string; }
