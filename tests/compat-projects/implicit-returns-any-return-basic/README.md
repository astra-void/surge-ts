# implicit-returns-any-return-basic

`noImplicitReturns` asks its question only of a function that is neither
`void`- nor `any`-returning, so `return <any>` on one path suppresses TS7030
while `return <unknown>` does not — `unknown` is a real value type, `any` is
not. surge reported both, because the void-like test recognised only `void`
and `undefined`.

The same test is what decides the question for a return expression surge could
not model. `Type::Unknown` is the degradation sentinel rather than the
`unknown` keyword (which is `GenuineUnknown`), so letting it answer turned an
unresolved type into a diagnostic: tRPC's `WsConnection.open` returns
`this.openPromise`, and under `SURGE_UPGRADE_ANALYSIS_SCOPES=1` that type
degrades and the method drew a TS7030 tsc does not report.

The async pair pins the unwrapping: `Promise<void>` is void-like through the
`async` return, a numeric one is not.
