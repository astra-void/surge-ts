# enum-member-constant-values-basic

tsc's `computeEnumMemberValues`: a member without an initializer continues a
numeric sequence, so it needs one after a string or computed member (TS1061);
a `const enum` initializer must be a constant expression (TS2474), and so
must a written initializer in an ambient enum (TS1066). Literals, earlier
members, module `const`s with constant initializers, imports and other
enums' members count as constant; `let`, `declare const`, calls and `as`
expressions do not. surge reported none of these.
