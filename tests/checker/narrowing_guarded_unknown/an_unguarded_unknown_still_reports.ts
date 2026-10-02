// An unguarded `unknown` still reports; the downgrade must not leak to the
// branch where the guard does not hold.
export function f(err: unknown) {
const message = err instanceof Error ? "" : err.message;
return message;
}
