declare function pickAny<T extends any>(a: T, b: T, c: T): T;
declare function pickUnknown<T extends unknown>(a: T, b: T): T;
declare function pickLiteral<T extends { x: 1 | 2 }>(a: T): T;

// `extends any` is read as `unknown`, and neither is a literal context: the
// literal members of an object literal argument widen as they do for an
// unconstrained parameter.
var fromAny = pickAny({ x: 3 }, { x: 6 }, { x: 6 });
var fromAny: { x: number };
var fromUnknown = pickUnknown({ x: 3 }, { x: 3 });
var fromUnknown: { x: number };
fromAny.x = 7;
fromUnknown.x = 7;

// A constraint that names literal members keeps them.
var fromLiteral = pickLiteral({ x: 1 });
var fromLiteral: { x: 1 };

const narrowAny: { x: 3 } = fromAny;
const narrowUnknown: { x: 3 } = fromUnknown;
