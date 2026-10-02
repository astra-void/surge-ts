// @surge-args: --diagnosticProfile native
// @surge-expect: surge::duplicate-type-parameter
interface Pair<T, T> { value: T; }
