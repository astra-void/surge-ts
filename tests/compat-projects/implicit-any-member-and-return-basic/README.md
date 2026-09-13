# implicit-any-member-and-return-basic

Under `noImplicitAny` a member with no type annotation is `TS7008` and a
signature with neither a body nor a written return type is `TS7010` — in a
class, an interface, a type literal, and an ambient function alike. An
unannotated type member is dropped entirely by the `Parsed*` lowering, so both
checks run in the parser's grammar walk.

`Widget.render` pins the overload half: the bodyless signature reports `TS7010`
even though the implementation right below it carries the body, and precisely
because it does, nothing reports `TS2391`.
