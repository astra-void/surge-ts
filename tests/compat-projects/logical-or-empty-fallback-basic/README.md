# logical-or-empty-fallback-basic

`(options || {}).color`: in a union, tsc reads a property an object-literal
member lacks as `undefined` rather than as missing, so the `{}` fallback idiom
reads `T | undefined`. surge already gave the empty fallback its sibling's
names on the `??` path and on the checked `||` path; the inferred `||` path —
the one a property access takes for its receiver — did not, which made the
direct read a false TS2339. A name no member has is still TS2339.
