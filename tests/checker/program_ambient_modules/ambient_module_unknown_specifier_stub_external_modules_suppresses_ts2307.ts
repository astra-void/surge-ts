// @surge-args: --stubExternalModules
// @surge-expect: none
// @filename: src/index.ts
import { missing } from "missing-pkg";
// @filename: types/pkg.d.ts
declare module "pkg" { export const foo: number; }
