// @surge-args: --diagnosticProfile native
// @surge-expect: none
// @filename: example.d.ts
// A wildcard module declaration is a supported shape (tsc reports nothing).
declare module "*" {}
