// The authoritative check pass re-registered every declaration and replaced,
// so only the last overload survived and a call matching an earlier one was a
// false TS2554.
declare function two(v: number): string;
declare function two(v: number, r: number): string;
export const a = two(1);
export const b = two(1, 2);
