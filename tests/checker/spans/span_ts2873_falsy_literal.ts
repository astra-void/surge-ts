// @surge-compare: spans
// The empty string, not `0`: tsc exempts numeric and boolean literal
// conditions from the always-truthy/always-falsy checks.
function f(): void { if ("") { } }
