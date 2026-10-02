// The left operand keeps contributing its non-nullish part to the result.
// @noImplicitAny: true
declare const maybe: number | undefined;
const value: number | string = maybe ?? "fallback";
const narrowed: number = value;
void narrowed;
