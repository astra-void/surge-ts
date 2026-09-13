# thenable-awaited-index-access-basic

`await` is erased at parse time, so a promise-typed value is modelled as the
value it resolves to everywhere — which is why `Promise<T>` indexes as `T`. A
class that *implements* `Promise<T>` rather than being one got no such
treatment, so `const rows = await db.execute(...)` reached the index as the raw
query object and reported a missing property.

drizzle's `QueryPromise` is that class, and every `PgRaw`/`SQLiteRaw` extends it:
four migrators do `const dbMigrations = await db.execute(...)` and then
`dbMigrations[0]`.

`firstRow` is that shape. `aPlainPromiseStillIndexes` is the control for the
path that always worked.

**Scope: index access only.** `rows.length` and `rows.map(...)` on the same
value still report — the awaited unwrap is applied where the receiver is
normalised for indexing, and the property-access path has several report sites
rather than one. Doing it properly wants `await` represented in the AST instead
of erased, which is also what the immediately-invoked-arrow parameters need; the
fixture deliberately does not pretend otherwise.
