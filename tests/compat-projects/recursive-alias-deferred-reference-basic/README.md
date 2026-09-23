# recursive-alias-deferred-reference-basic

A type alias may recurse through the type arguments of a generic interface
reference: tsc creates such a reference deferred (`isDeferredTypeReferenceNode`),
so React's `type ReactNode = … | Iterable<ReactNode> | …` is a valid recursive
alias. surge accepted recursion only through a structural literal in the body,
treated this one as a circular alias, and left `ReactNode` — and every React
type that mentions it — at the degradation sentinel.
