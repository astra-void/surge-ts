// The alias is forced from a call's explicit type argument, not from a
// variable annotation — the body's value scope has to be visible wherever the
// declaration is first resolved from.
declare function expectTypeOf<T>(): void;
export function f() {
const schema = { name: "x" };
type Schema = typeof schema;
expectTypeOf<Schema>();
}
