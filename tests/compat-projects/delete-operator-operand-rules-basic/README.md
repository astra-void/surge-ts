# delete-operator-operand-rules-basic

The parser lowered `delete` to the same `Discard` operator as `void` and `~`,
so none of the operand rules tsc's `checkDeleteExpression`
(`tsc/internal/checker/checker.go`) applies had anywhere to run, and the whole
expression was typed as unmodelled rather than `boolean`.

`delete` now keeps its own operator. The operand must be a property reference
(TS2703, paired with the strict-mode syntax error TS1102 that the grammar pass
reports on a bare binding), must not be `readonly` (TS2704), and must admit
`undefined` (TS2790). An index-signature element and a member of `any` name no
property symbol, so — as in tsc — no operand rule applies to either.

`delete this.#x` is deliberately **not** pinned here. tsc reports TS18011 and
TS2790; surge reports neither, because a private field is not parsed at all.
Reporting the *shape* rule on that unmodelled operand would be a false TS2703,
so the operand rules skip an operand surge did not model.

tsc gates TS2790 on `strictNullChecks`; surge models no such flag and checks as
if it were always on, matching the rest of its possibly-undefined reporting.

`if (o.p) delete o.p` is pinned because the guard narrows `p` to a non-nullish
type, which would make the operand read as required.
`checkDeleteExpressionMustBeOptional` asks `getTypeOfSymbol` — the member's
*declaration* — so the rule walks back to the binding's pre-narrowing type
instead of using the flow type at the operand.
