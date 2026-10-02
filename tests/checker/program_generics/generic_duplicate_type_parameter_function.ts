// @surge-args: --diagnosticProfile native
// @surge-expect: surge::duplicate-type-parameter
function identity<T, T>(value: T): T { return value; }
