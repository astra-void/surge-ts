// Distribution must not silently widen the target: every arm still requires
// the non-union operand's members.
type U = { data: number } | { error: string };
type I = { request: string };
export const c: U & I = { data: 1 };
