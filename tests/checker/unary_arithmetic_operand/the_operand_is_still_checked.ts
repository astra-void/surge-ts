// The operand is still an expression: dropping the report must not stop it
// being checked.
type Plain = { value: string };
declare const g: Plain;
export const a = +g.missing;
