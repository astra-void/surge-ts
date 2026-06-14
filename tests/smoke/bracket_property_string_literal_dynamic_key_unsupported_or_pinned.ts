// Dynamic (non-string-literal) keys are intentionally out of scope for the
// string-literal bracket-access slice: `config[key]` is NOT lowered to property
// access and does not resolve to a declared property. This fixture pins the
// existing index-access behavior so the dynamic-key path stays unchanged.
const config = { secret: "value" };
let key = "secret";
config[key];
