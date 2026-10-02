// @surge-compare: spans
// A literal index on a primitive receiver is never a TS2339. tsc resolves it
// against the primitive's apparent type: `string` carries a numeric index
// signature and `number`/`boolean` have none, so under `noImplicitAny: false`
// the access is implicitly `any` with no diagnostic at all.
let value = 1; value[0];
