# missing-return-bare-return

tsc's binder sets `hasExplicitReturn` for every `return` statement, a bare
`return;` included, while a `throw` leaves it unset. So a function whose only
return is `return;` is not TS2355 — its reachable end falls through to the
`noImplicitReturns` TS7030 (no `strictNullChecks`, so not TS2366) — whereas
one that only throws is.
