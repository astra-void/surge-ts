// @surge-compare: spans
// tsc anchors a variable-initializer assignability error on the declaration
// name, not the initializer value.
let value: number = "a";
