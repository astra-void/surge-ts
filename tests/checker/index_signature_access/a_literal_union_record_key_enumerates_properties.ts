// A literal-union key still enumerates properties, numeric literals included.
// @noImplicitAny: true
declare const rec: Record<1 | 2, boolean>;
export const a: boolean = rec[1];
export const b = rec[3];
