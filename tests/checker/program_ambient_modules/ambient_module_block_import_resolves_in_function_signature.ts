// @filename: types/fsp.d.ts
// The imported block sits in a later file, so it is not registered yet
// when the importing block's signatures are first seen.
declare module "m:fsp" {
    import { PathLike } from "m:fs";
    function access(path: PathLike): void;
}
// @filename: types/fs.d.ts
declare module "m:fs" {
    type PathLike = string;
}
// @filename: src/index.ts
import { access } from "m:fsp";
access("a");
access(1);
