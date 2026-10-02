// @filename: src/index.ts
import { foo } from "pkg";
// @filename: types/pkg.d.ts
declare module "pkg" { export const foo: number; }
