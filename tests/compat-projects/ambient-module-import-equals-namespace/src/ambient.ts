declare module "lib2" {
    export class D { y: string; }
    export var v: number;
    export function f(): string;
    export namespace N { export interface I { i: number } }
}
declare module "user" {
    import lib = require("lib2");
    export var fromLib: lib.D;
    export var fromLib2: typeof lib;
    export var nested: lib.N.I;
    export var asType: lib;
}
