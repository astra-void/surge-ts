// @filename: src/index.ts
import * as pkg from "pkg-default"; let ok: string = pkg.default;
// @filename: types/pkg-default.d.ts
declare module "pkg-default" { export const value: string; export default value; }
