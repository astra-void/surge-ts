// @surge-args: --diagnosticProfile native
// @surge-expect: none
// @filename: types/globals.d.ts
// Identifier-named namespaces are parsed so their members (e.g.
// `JSX.IntrinsicElements`) can resolve; an empty namespace is inert, matching
// tsc which reports nothing here.
declare namespace N {}
