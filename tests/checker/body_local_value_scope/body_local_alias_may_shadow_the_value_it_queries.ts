// A body-local alias may shadow the value it queries (zod's `type a = Infer<typeof a>`).
export function f() {
const a: { q: number } = { q: 1 };
type a = typeof a;
const branded = (_: a) => {};
branded({ q: 2 });
}
