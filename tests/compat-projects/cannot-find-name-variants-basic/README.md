# cannot-find-name-variants-basic

tsc's `onFailedToResolveSymbol` and `getCannotFindNameDiagnosticForName`: a
name that resolves to nothing is reported as TS2304 only when nothing more
specific applies. In tsc's order: a class member that would resolve with a
`this.`/`C.` prefix (TS2663, TS2662), a type used as a value (TS2693), a value
or namespace used as a type (TS2749, TS2709), a type used as a namespace
(TS2713, TS2702), then a lib hint, a spelling suggestion (TS2552, TS2833), and
the name-specific message — node, test-runner, jQuery and Bun globals
(TS2591, TS2593, TS2592, TS2868), `await(…)` outside an async function
(TS2311), a shorthand property (TS18004) — before plain TS2304 or TS2503.
