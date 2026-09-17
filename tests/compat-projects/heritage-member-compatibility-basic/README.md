# heritage-member-compatibility-basic

Only the *missing member* half of the heritage check existed (TS2420), so a
member that was present but mistyped went unreported.

tsc's shape here is one gated flow, not independent checks
(`issueMemberSpecificError`): it first walks the class's own non-static members
and reports each one not assignable to the same-named base member (TS2416,
anchored on the member name), and falls back to the broad diagnostic **only if
that walk reported nothing**. `BadAndMissing` pins that gate — it both mistypes
`area` and omits `name`, and tsc reports only the TS2416.

The accepting cases are pinned because they are what the rule is easy to get
wrong on: narrowing a property (`p: 5` against `p: number`), keeping a member
optional, and adding members the base never had are all fine, and a method
whose parameter merely differs is still an error when neither direction
relates (`m(x: string)` against `m(x: number)`).

Two restrictions keep this free of false positives, and both are deliberate:
a generic class is skipped, and so is a heritage clause carrying type arguments
— resolving the base by name alone would compare the class against the base's
*uninstantiated* members (`v: number` against `v: T`).

TS2415 (the broad `extends` diagnostic) is **not** implemented. The base-class
side has no missing-member notion to pair it with, and it would have to trust a
whole-type relation against a class surface surge does not model completely.

Every member here is **annotated deliberately**. An unannotated class property
takes its type from its initializer in tsc but is typed `any` by surge's
instance surface (`class_member_to_interface_member`), so
`class D extends Base { p = "s" }` is a missed TS2416 — a pre-existing
inference gap, not a property of this rule. Pinning the annotated form keeps
the fixture about the heritage check; closing that gap belongs with the
unannotated-initializer work.

