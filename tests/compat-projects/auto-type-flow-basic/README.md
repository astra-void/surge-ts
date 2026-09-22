# auto-type-flow-basic

tsc types an un-annotated `let x;` (or `= undefined` / `= null`) by what is
assigned to it (`autoType`), and a closure continues the enclosing flow only
for a `let` past its last assignment — never from a nested `function`
declaration. A closure that cannot see the flow reads an implicit `any`
(TS7005/TS7034), or `undefined` when nothing ever assigns it.

Assignments inside expressions take part in that flow and in definite
assignment: `if ((x = f()))`, `c && (x = 1)` (conditional), `c ? (x = 1) :
(x = 2)` (both branches), `a = b = 1`, `x ??= 1`, and destructuring
assignment, including in a `while` condition.
