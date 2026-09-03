# optional-property-conditional-undefined-basic

Pins that a conditional value assigned to an **optional** property is accepted:
without `exactOptionalPropertyTypes` an optional property's type includes
`undefined`, so `flag ? value : undefined` is assignable and `tsc` is silent.

Before the fix, each branch of the conditional was checked against the *bare*
declared property type and the `undefined` branch produced a surge-only TS2322.
That was the last open false positive on the `unnamed` corpus
(`data-table.tsx:649`, a JSX attribute `onAutoEditConsumed={flag ? … : undefined}`).

The last line keeps the required/optional distinction honest: `undefined` against
a **required** property is still TS2322, at the same span and message as `tsc`.

The over-correction guard for the conditional path — a wrong-typed branch such
as `flag ? "x" : undefined` must still report — lives as a Rust test rather than
here, because surge anchors a conditional's diagnostic at the failing branch
while `tsc` anchors it at the property value, and a fixture would only add
non-gating span/message drift.
