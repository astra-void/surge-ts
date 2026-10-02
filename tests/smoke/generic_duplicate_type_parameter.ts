// @surge-args: --diagnosticProfile native
// @surge-expect: surge::duplicate-type-parameter
type Pair<T, T> = [T, T];
