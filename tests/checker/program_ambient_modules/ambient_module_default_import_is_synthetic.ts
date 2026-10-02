// @filename: src/index.ts
// tsc: a declaration file's module can always be imported through a
// synthetic default.
import value from "pkg-default";
// @filename: types/pkg-default.d.ts
declare module "pkg-default" { export const value: string; }
