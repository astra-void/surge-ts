// @surge-compare: spans
// @filename: types/pkg.d.ts
declare module "pkg" { export const foo: number; }
// @filename: example.ts
// tsc: a declaration file's module can always be imported through a
// synthetic default.
import foo from "pkg";
