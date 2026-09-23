export namespace Outer {
    import lib = require("ambient-lib");
    import missing = require("./not-there");
    import real = require("./real");
    import unused = require("./also-not-there");

    export const fromAmbient: string = lib.value;
    export const fromMissing = missing.anything;
    export const fromReal = real.real;
}
