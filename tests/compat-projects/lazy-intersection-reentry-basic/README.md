# lazy-intersection-reentry-basic

A deferred intersection merge whose members lead back to the same intersection
while it is still being merged. Reduced from `@xata.io/client` (drizzle-orm's
`xata-http` driver) by delta-debugging: `BaseClient` extends a construct
signature returning `Omit<{ search: … }, …> & {}`, and resolving that reaches
`XataRecord` → `Readonly<SelectedPick<…>>` → the same merge.

The merge memo was a `OnceLock` initialized re-entrantly on one thread, and
std's `Once` blocks on itself: surge stopped using CPU and never exited. The
back-edge now reads the degradation sentinel, the way a blocked lazy peel does,
and a merge that consumed it is not memoized.

The declaration file is the reduction as found, unresolved names included;
`skipLibCheck` keeps tsc from reporting them. The only expected diagnostic is
the `TS2322` in `src/index.ts`, which proves the check ran to completion.
