// @noImplicitAny: true
type ByKey<K extends string | number> = { [P in K]: boolean };
declare const byLiteral: ByKey<1 | 2>;
export const a: boolean = byLiteral[1];
export const b = byLiteral[3];
