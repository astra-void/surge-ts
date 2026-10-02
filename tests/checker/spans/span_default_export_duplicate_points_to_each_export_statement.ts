// @surge-args: --diagnosticProfile native
// @surge-expect: TS2528 TS2528
// tsc reports every default export, not only the later one.
export default 123;
export default 456;
