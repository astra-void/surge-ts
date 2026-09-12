# predicate-type-argument-from-arguments-basic

A generic predicate's type arguments were inferred from the *tested* argument
only. Anything the predicate states in terms of another argument —
`isOfKind(kind, value): value is Extract<T, { kind: K }>`, where `K` is the
first argument — left that parameter unbound, and the fallback filled it with
`any`. `Extract<T, { kind: any }>` then matches the wrong member, so the true
branch narrowed to the square and *both* branches reported.

Each argument is now inferred against its own declared parameter, and only while
something is still unbound, so a predicate whose parameters the tested argument
already fixes does no extra work. `fromTheTestedArgumentAlone` is that control.

`theOtherBranchIsTheOtherMember` is the intentional error and pins the
direction: the false branch really is the square, so reading `radius` there
reports.

This is the second of the two layers under ts-pattern's last remaining
diagnostic. With it, `isMatching`'s `P` binds to the pattern argument instead of
`any` and the predicate resolves rather than being abandoned — but that corpus
does not move, because the pattern's own type comes back degraded from an
overload group (`P.array()` is `any`), and ts-pattern's `InvertPattern` on a
degraded pattern yields `never`. Closing that is the blocked overload-resolution
program, not this fixture.
