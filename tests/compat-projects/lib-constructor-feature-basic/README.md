# lib-constructor-feature-basic

tsc's `getScriptTargetFeatures` is keyed by the interface a member would have
been declared on, and that covers the lib's constructor objects too: under
`target: es2015`, `Object.entries` / `Object.fromEntries` / `Object.hasOwn` /
`Promise.allSettled` are TS2550 ("Do you need to change your target
library?"), not TS2339. surge had the table for arrays and strings only, so the
code differed. Members the configured lib does declare (`Object.keys`,
`Array.from`, `Math.trunc`, `Promise.resolve`) stay valid, and a name no lib
declares is still TS2339.
