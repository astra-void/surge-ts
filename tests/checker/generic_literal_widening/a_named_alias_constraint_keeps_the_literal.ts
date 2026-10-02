// A named alias for a literal union is indistinguishable from any other alias
// without resolving it, so a constrained parameter never widens: widening this
// one collapsed `Field` to `string` and made `Row[Field]` an invalid index.
type Row = { name: string; age: number };
type Editable = "name" | "age";
declare function update<Field extends Editable>(field: Field, value: Row[Field]): void;
export function f() {
update("name", "x");
}
