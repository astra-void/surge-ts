# ambient-global-function-overload-merge

A global function re-declared by another declaration file is an *overload* of the
same name, not a shadow. `@types/node` re-declares `clearTimeout` for
`NodeJS.Timeout` alongside the DOM lib's numeric one, and first-wins ambient
value insertion kept only whichever came first — so `clearTimeout(timer)` on a
`NodeJS.Timeout` reported TS2345 against the numeric signature.

The fixture reproduces it without Node or the DOM: one declaration file declares
`stopTimer(handle?: number)`, another contributes `stopTimer(handle: Fake.Handle
| undefined)` through `declare global`, and both call forms must check.

Only the two accepting call forms are pinned. The merge folds the signatures
into one *permissive* shape rather than resolving overloads — the same trade
`merge_overload_signatures` already makes for an intersection of function types
— so an argument matching neither overload is currently accepted where tsc
reports TS2769. Real overload resolution is the separate, CPU-blocked piece of
work; until it lands, pinning that row here would only record a known
under-report as if it were expected.
