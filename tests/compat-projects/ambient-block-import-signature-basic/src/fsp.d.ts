declare module "m:fsp" {
    import { PathLike, Stats } from "m:fs";
    interface Handle { stats: Stats }
    function access(path: PathLike): void;
    function stat(path: PathLike): Promise<Stats>;
    function missing(path: NotDeclared): void;
}
