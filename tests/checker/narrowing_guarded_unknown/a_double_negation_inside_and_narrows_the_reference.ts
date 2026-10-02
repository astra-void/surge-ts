// `!!x` as an `&&` operand proves `x` the way the bare reference does, also
// when it reaches the condition through a boolean `const` alias — tanstack's
// `const isRefetch = !!query && query.isFetched(); if (isRefetch && mode ===
// 'reset') { query.setState(…) }`. Only the reference-guard walk lacked the
// unwrap: `!!query` on its own already narrowed.
interface Query { isFetched(): boolean; setState(s: object): void }
declare function find(): Query | undefined;
export function inline(mode: string) {
const query = find();
if (!!query && mode === 'reset') { query.setState({}); }
}
export function aliased(mode: string) {
const query = find();
const isRefetch = !!query && query.isFetched();
if (isRefetch && mode === 'reset') { query.setState({}); }
}
