# nominal-enum-display-basic

An enum type is nominal in tsc and displayed by the enum's name. surge lowers an
`enum` to a literal-union alias plus one alias per member, which cannot express
that on its own, so the resolution is wrapped in a nominal `Type::Reference`
whose *display* carries the enum while the payload stays the literal union
assignability already understands.

Both halves of tsc's formatting rule are pinned here: an **exported** enum is
qualified with the module it came from (`import("…/src/index").Exported`), a
file-local one is named bare (`Local`). A single member displays as the enum,
not as `Enum.Member`.

The last two declarations pin what must *not* change: an enum member is still
assignable to its underlying primitive, and discriminant narrowing on a string
enum member still selects the right union member.

`crates/surge-ts-checker/tests/nominal_enum.rs` pins what the wrapper must not
break: a member stays assignable to its own member type and to its underlying
primitive, a numeric member still accepts a plain number, and a mismatch against
an unrelated type still reports.

**Not implemented: nominal enum *assignability*.** tsc rejects a bare literal
against a *string* enum member (`const c: Names.Wide = "wide"`) and rejects a
cross-enum member. Both need the enum member type to survive a *value* read —
today the lowered `const Names` object types its properties as the raw literals,
so `Names.Wide` as a value is indistinguishable from `"wide"`. Pointing those
properties at the member aliases was tried and reverted: every site that reads a
discriminant out of the enum object compares literals directly, so narrowing
inverted (`v.kind === Names.Wide` stopped selecting) and the cross-member
mismatch was still missed. Closing it means teaching each of those comparisons
to peel, not just changing the lowering.
