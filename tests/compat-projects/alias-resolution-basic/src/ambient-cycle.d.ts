declare module "ambient-one" {
    import two = require("ambient-two");
    export = two;
}
declare module "ambient-two" {
    import one = require("ambient-one");
    export = one;
}
