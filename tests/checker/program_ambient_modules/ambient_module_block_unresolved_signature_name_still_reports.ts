// @surge-compare: messages
// @filename: types/fsp.d.ts
declare module "m:fsp" {
    import { PathLike } from "m:fs";
    function access(path: PathLike): void;
    function missing(path: NotDeclared): void;
}
// @filename: types/fs.d.ts
declare module "m:fs" {
    type PathLike = string;
}
// @filename: src/index.ts
export {};
