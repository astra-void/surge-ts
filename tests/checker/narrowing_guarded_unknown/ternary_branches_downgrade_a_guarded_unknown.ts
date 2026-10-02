// Ternary branches see the guard exactly as the `if` form does.
export function f(err: unknown) {
const message = err instanceof Error ? err.message : "";
const length = typeof err === "string" ? err.length : 0;
return [message, length];
}
