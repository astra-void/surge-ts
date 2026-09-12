# lib-reference-index-and-signature-scope-basic

- A nominal array reference (`Array<number>`, `ReadonlyArray<string>`) indexes
  like the array it names; surge fell through to the object arm and reported
  the *receiver* as a missing property.
- `Promise<T>` is modelled as its awaited `T`, so an optional continuation
  (`result?.catch(...)`) on a `Promise<void> | undefined` receiver landed on
  `void` and reported a missing member; it now continues the way the
  non-optional call does.
- `typeof <parameter>` in a later parameter's annotation (or the return
  annotation) resolves against the parameters already bound, as tsc's
  parameter scope does; surge reported the name as unknown.

The intentional error pins the `| undefined` element read under
`noUncheckedIndexedAccess`.
