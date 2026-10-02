// @filename: src/index.ts
import getName from "pkg-default-function"; let name: string = getName();
// @filename: types/pkg-default-function.d.ts
declare module "pkg-default-function" { export default function getName(): string; }
