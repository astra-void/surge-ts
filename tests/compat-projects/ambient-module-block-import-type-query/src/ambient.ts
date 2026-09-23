declare module "lib4" {
    namespace L { var count: number; }
    export = L;
}
declare module "lib5" {
    export var w: number;
}
declare module "user" {
    import lib4 = require("lib4");
    import * as lib5 from "lib5";
    export var fromLib4: typeof lib4;
    export var fromLib5: typeof lib5;
    export var count: typeof lib4.count;
}
