# library-reference-outside-generic-signature

A type reference to a dependency's declaration resolves that declaration in
its own lexical scope. The type parameters of the signature that writes the
reference (`parentOfType<TPath>(type: Api['Declaration'])`) are not visible
there, so how the library's instantiations expand cannot depend on which file
wrote a generic signature naming them.

surge peeled such a reference under the writing site's type parameter scopes,
which made every nested instantiation non-concrete: `Collection<T>` inside
`TraversalMethods.find` expanded eagerly, cut its own cycle to `unknown`, and
that expansion was interned for every consumer. `use.ts` then lost the type of
`find(...)` and its TS2322, but only when `signature.ts` was in the program.
