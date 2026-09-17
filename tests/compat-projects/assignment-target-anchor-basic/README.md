# assignment-target-anchor-basic

tsc checks `x = value` with `checkTypeAssignableToAndOptionallyElaborate`, whose
error node is the *left* operand: a mismatch is reported at the target, and only
moves into the value when `elaborateError` can point at something inside it —
an object or array literal member. surge anchored identifier assignments
(including compound ones, and the TS2741 missing-property form) on the value.

A member write (`holder.p = "s"`) and an elaborated object-literal member
(`shape = { a: "s" }`) already anchored correctly and are pinned alongside.
