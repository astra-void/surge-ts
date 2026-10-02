// @filename: src/index.ts
import * as pkg from "pkg-ns"; let user = pkg.User;
// @filename: types/pkg-ns.d.ts
declare module "pkg-ns" { export interface User { name: string; } }
