# interface-method-overload-selection-basic

A call through an overloaded interface method returns the first overload that
accepts its arguments (tsc's `chooseOverload`). surge folded such a group into
one permissive signature and never attached the members, so every call through
it was silent — `parser.parse(['x'])`, `Object.fromEntries(...)`, and the index
signature read (TS4111) its result makes.

- A generic candidate is instantiated for the call before it is tested; its
  literal argument stays a literal when the contextual type names it
  (`Promise.resolve('data')` against `Loader<'data'>` is not an error).
- A `const` type parameter infers an array literal as a tuple.
- `Symbol.iterator in headers` narrows by the well-known symbol key, so the
  iterable arm reaches `Object.fromEntries` and no overload mismatch is reported.
