// A tuple-typed rest parameter (`...args: [name: string]`) — and a union of
// tuples, next's cookie-store overload shape — accepts each argument at its
// tuple position instead of comparing the whole tuple/union against every
// argument. Only the genuinely mismatched call reports.
declare function get(...args: [string] | [{ name: string }]): void;
get("NEXT_LOCALE");
get({ name: "lang" });
get(123);
