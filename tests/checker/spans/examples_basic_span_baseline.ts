// @surge-compare: spans
// @filename: examples/basic.ts
// `var a: string = 1;` — tsc anchors the assignability error on the
// declaration name `a`, not the initializer.
var a: string = 1;
