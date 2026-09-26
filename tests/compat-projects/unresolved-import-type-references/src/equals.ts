import Missing = require("missing-module");

// A reference through an import whose module does not resolve is the error
// type: a callback typed by it has no contextual signature.
export const handler: Missing.Handler<number> = (event) => event;
export function make<T>(): Missing.Factory<T> {
    return (options) => options;
}

