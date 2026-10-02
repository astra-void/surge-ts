// @filename: src/index.ts
import { foo } from "pkg/subpath";
// @filename: types/pkg.d.ts
declare module "pkg" { export const foo: number; }
