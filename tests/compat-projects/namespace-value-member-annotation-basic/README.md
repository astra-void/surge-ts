# namespace-value-member-annotation-basic

A `declare namespace`'s *value* members carry their written annotation into the
namespace object, so `ns.member` has a real type instead of `any`.

The object is what a consumer reads — directly, through a namespace import, and
through an intersection that includes `typeof ns` — and every member was built
permissively, so nothing downstream of one could be checked. Sibling names
resolve under the namespace prefix, the same way a member *signature* already
did; a member whose annotation does not resolve keeps the permissive type
rather than degrading.

`ast-types` declares its whole named-type and builder surface this way
(`let VariableDeclaration: Type<VariableDeclaration>`), which is how
jscodeshift's `j.VariableDeclaration` reached tRPC's `upgrade` transforms.
