// @surge-compare: spans
// Two missing properties: tsc lists both and reports TS2739. One missing
// property would be TS2741 on the same span.
let user: { name: string; age: number } = {};
