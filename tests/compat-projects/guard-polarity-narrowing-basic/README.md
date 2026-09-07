# guard-polarity-narrowing-basic

Three narrowing gaps that only show up on the *false* side of a guard.

`if (A || B || C) return;` proves every disjunct false in the fall-through, so
each disjunct's negation is a guard that holds. The same chain narrows
*within* itself: `b` in `a || b` only runs when `a` is falsy, so the fourth
disjunct here sees `error` already guarded by the first three.

`isObject(value) === false` is the same guard as `!isObject(value)`; comparing
a call against a boolean literal must not lose the narrowing.

`Object.prototype` members resolve on the apparent type rather than through an
index signature, so `value.constructor` on a `Record<string, unknown>` is a
plain property access — not `TS4111` under
`noPropertyAccessFromIndexSignature`.

The single intentional error is the last function: with no guard at all the
receiver is still `unknown`, so the access is a `TS18046`.
