// @surge-args: --diagnosticProfile native
// @surge-expect: TS2314
type Box<T> = { value: T }; let box: Box = { value: "ok" };
