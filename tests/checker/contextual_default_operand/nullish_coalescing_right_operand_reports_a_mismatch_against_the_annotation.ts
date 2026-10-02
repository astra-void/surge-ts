// The contextual type still has to be honoured: a right operand that does not
// fit the annotation is reported rather than silently accepted.
// @noImplicitAny: true
declare const override: number | undefined;
const value: number = override ?? "fallback";
void value;
