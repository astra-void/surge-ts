export namespace Outer {
    export import exported = require("first");
    import local = require("second");
}
