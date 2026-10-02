// @surge-compare: spans
// @filename: types/pkg.d.ts
declare module "pkg" { export const foo: number; }
// @filename: example.ts
import * as pkg from "pkg"; let value = pkg.missing;
