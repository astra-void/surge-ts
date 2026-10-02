// @surge-args: --diagnosticProfile native
// @surge-expect: none
// @filename: types/globals.d.ts
// Enums lower to a member-literal union plus an ambient value binding; an
// empty one is inert, matching tsc which reports nothing here.
declare enum E {}
