class C { test: string = "" }
class D extends C { test2: string = "" }
declare function testRest<T extends C>(a: (t: T, t1: T, ...ts: T[]) => void): T;
declare function pickDefault<T = string>(cb: (x: T) => void): T;

// Typing a callback parameter fixes the type parameters its contextual type
// names; one no candidate names is fixed at its default, else its
// constraint, else `unknown`.
testRest((t1, t2, t3) => {});
const fromConstraint = testRest((t1, t2) => {});
const fromConstraintCheck: D = fromConstraint;
const fromDefault = pickDefault((x) => { x.toUpperCase(); });
const fromDefaultCheck: string = fromDefault;

// An annotated parameter infers before the others are typed.
testRest((t1: D, t2, t3) => { t2.test2; });
testRest((t1, t2: D, t3) => { t1.test2; });
testRest((t2: D, ...t3) => { t3[0].test2; });

// The contextual return type infers first.
const promised: Promise<number> = new Promise((resolve) => resolve(1));
