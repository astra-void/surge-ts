# ambient-namespace-member-contextual-basic

An ambient `declare namespace`'s value surface is built by
`fill_namespace_value_properties`, which gave every function member a
zero-parameter variadic stub. Nothing lines an argument up with a parameter
there, so an object literal written against the namespace's own type — the
`globalThis.awslambda = { streamifyResponse(handler) {…} }` shape AWS's Lambda
runtime types ask for — left the method's parameters untyped and reported a
false TS7006 on each.

The stub is now spelled `(...args: any[]) => any`, so a parameter position
exists and is contextually `any`, which is what tsc gives a method written
against a rest-`any` signature.

The stub is still a stub: the members' **written** signatures are not mapped, so
a *call* through such a member (`hostruntime.onError(err => …)`) still types its
callback parameters from the rest parameter rather than from the declaration.
That gap is deliberate and unfixed here — mapping those signatures means
resolving namespace-scope annotations during ambient lowering, which runs before
`declare global` blocks are lowered.
