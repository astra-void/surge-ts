# generic-type-predicate-narrowing-basic

Two user-defined type-predicate shapes that did not narrow before.

**A generic predicate** (`<T>(x: AnyResult<T>) => x is OK<T>`, zod's `isValid`)
was rejected outright: an unsubstituted `T` cannot be resolved at the guard site.
It is now inferred from the tested argument's own type. When the written
parameter is an *alias* whose expansion is a union there is nothing to align the
alias's argument against, so `T` stays unbound; the predicate is still usable
because narrowing only *filters* the subject's union members, and the surviving
members keep their own type arguments. That fallback is taken only when the
filled predicate genuinely selects among the members — never when narrowing
would substitute the predicate wholesale.

**A predicate written as a `const` annotation** (`declare const f: (x: S) => x is
OK`) carried no collected signature, so the guard was never recognized. The
annotation is now kept for the same reason a generic one is: the predicate is
not recoverable from the resolved callable type.

The last function pins the false branch: the non-matching member survives, so
`r.status` still resolves.

Still open: a predicate declared as an *object-type method*
(`declare const helpers: { isValid(x: S): x is OK }`) is not recognized.
