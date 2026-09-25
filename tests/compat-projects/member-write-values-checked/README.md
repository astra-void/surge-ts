# member-write-values-checked

The value of a member write is checked whatever becomes of the write itself
(`checkBinaryLikeExpression` checks both operands): through a read-only
property (TS2540), a member the receiver lacks (TS2339), a namespace import's
member, or an `any` receiver, the value's own errors are still reported. A
function value checked that way may read a member a later write declares on a
function of the same body (an expando, which tsc's binder declares up front),
and sees it.
