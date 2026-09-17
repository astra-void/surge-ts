# property-initialization-basic

`strictPropertyInitialization` (TS2564) is the single most frequent diagnostic
in tsc's own conformance baselines that surge could not emit. It needed three
things that were missing: the `declare` and `!` modifiers on a class property
(oxc reports both; surge's AST dropped them), a `strictPropertyInitialization`
option, and a notion of "definitely assigned in the constructor".

tsc answers that last one with its flow graph — it types a synthesized `this.p`
reference at the constructor's end and checks it no longer includes
`undefined`. There is no such graph here, so the rule is re-derived over the
statement list the way `crate::flow` derives `guarantees_exit`, and the cases
that separate the two readings are all pinned:

- `if`/`else` assigning in both branches initializes; a lone `if` does not.
- A branch that cannot complete (`return`, `throw`) leaves the other as the
  only path, so an assignment after it still counts.
- A loop body may run zero times, so it does not count. (`do … while` lowers
  with `runs_at_least_once` and does.)
- Delegating to a helper method does not count — tsc only looks at the
  constructor.
- Of a constructor overload set, only the implementation carries statements.

`readonly` is deliberately in the reporting half: it exempts nothing, which is
easy to assume it does.

Computed property names (`["computed"]: string`) are **not** pinned, but they do
reach the check — they diverge only in how they are *displayed*. tsc names the
member by its source text (`'["computed"]'`) and anchors on the opening bracket;
surge carries a string-literal computed key as the plain name (`'computed'`) and
anchors inside the bracket. Same code, same line, so the gate is unaffected;
this is the display-identity drift tracked elsewhere, not a gap in this rule.

A property of a **class expression** (`const C = class { p: string }`) is also
missed: tsc checks it, surge's class checking runs over declarations only.
