// @filename: src/index.ts
import value from "pkg-default"; let ok: string = value;
// @filename: types/pkg-default.d.ts
declare module "pkg-default" { export const value: string; export default value; }
