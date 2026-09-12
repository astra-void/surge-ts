# overload-group-type-predicate-basic

An overload group keeps **one** declaration's parsed signature — the first — and
guard narrowing reads the type predicate off exactly that signature. A group
whose predicate is written on a *later* overload therefore had no predicate at
all as far as narrowing was concerned, and `if (match('circle', shape))` narrowed
nothing. Both branches then read the un-narrowed union, so both reported.

Swapping which declaration the group keeps is not the fix: the kept signature is
also the instantiation template, and promoting the predicate overload made
`match('circle')` — the *other* overload, whose return is a function — report as
not callable. The predicate-bearing overload is carried alongside the kept one
instead, in a field only guard narrowing reads, so the folded value type and
every instantiation path are untouched.

`theOtherOverloadStillReturnsItsOwnType` is the control for exactly that: the
one-argument call must keep returning its function. Which overload a call
actually reaches is still decided the same way as before — the predicate's
parameter name maps to an argument position, and a call that does not reach that
position narrows nothing.

`theFalseBranchIsTheOtherMember` is the intentional error and pins the direction
of the narrowing: the false branch really is the square, so reading `radius`
there reports.

ts-pattern's `isMatching` is the shape this came from. Its predicate sits on the
second of three declarations, and finding it moved that corpus's last remaining
diagnostic one layer deeper — the predicate is now found and resolved, and what
fails is evaluating `T & WithDefault<P.narrow<T, P>, P.infer<P>>` when `P` was
never inferred from the pattern argument.
