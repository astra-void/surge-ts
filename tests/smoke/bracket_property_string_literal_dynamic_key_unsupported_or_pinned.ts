// Dynamic (non-string-literal) keys are intentionally out of scope for the
// string-literal bracket-access slice: `config[key]` is NOT lowered to property
// access and does not resolve to a declared property.
//
// tsc reports TS7053 here (implicit-any element access: `string` can't index
// `{ secret: string }`, which has no index signature) — NOT TS2339. surge does
// not yet model TS7053, but it must not emit the old false-positive TS2339 that
// mis-named the receiver as the missing property, so the expected set is empty.
const config = { secret: "value" };
let key = "secret";
config[key];
