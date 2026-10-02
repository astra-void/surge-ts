// @surge-args: --diagnosticProfile native
// @surge-expect: none
// @filename: types/globals.d.ts
// `declare class` is now supported: it contributes a global value/type and
// should not produce an unsupported-declaration diagnostic.
declare class Foo {}
