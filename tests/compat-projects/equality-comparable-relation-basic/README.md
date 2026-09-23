# equality-comparable-relation-basic

tsc checks `==`, `!=`, `===` and `!==` with `isTypeEqualityComparableTo` in
either direction (`checkBinaryLikeExpression`, checker.go): the other operand
is `undefined` or `null` as a whole, or the two types are comparable
(relater.go). surge decided TS2367 by sorting both operands into kinds and
never reported two object, function, array or class types, so `Base` against
an unrelated class, `{ fn(): Base }` against `{ new (): Base }` or
`Promise<string>` against `string` went unreported, while `[string, number]`
against `string[]` — comparable, since some element relates — was a false
positive.

The comparable relation also decides the enum and literal cases (`E` against a
value it has no member for, a template literal against a literal its fixed
text rules out) and type variables of a generic body: two of them overlap only
when one is constrained to the other. A `case` test is compared to the
discriminant under the same relation (TS2678, `switches.ts`).

`intersections.ts` pins intersection operands on either side: an intersection
source relates when some constituent does or its members do as a whole, an
intersection target needs every constituent, so `A & B` overlaps `A` but not
`B & C`, and `I1 & I3` has no overlap with `I2 extends I1` in either direction.

`weak.ts` pins the one weak-type rule the comparable relation keeps
(`isPerformingCommonPropertyChecks`, relater.go): a unit source — a literal,
an enum member, or `boolean` as `false | true` — has to share a property with
an all-optional target, while `string` against the same target is comparable.
