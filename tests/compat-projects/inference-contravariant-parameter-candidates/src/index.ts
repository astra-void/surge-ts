interface Base { a: number }
interface Derived extends Base { b: string }
declare const base: Base;
declare const derived: Derived;
declare function on<T>(handler: (e: T) => void, other: (e: T) => void): T;
declare function emit<T>(value: T, handler: (e: T) => void): T;
declare function tag<T>(x: T, a: (x: T) => T, b: (x: T) => T): T;
declare function withMethod<T>(value: T, handlers: { handle(e: T): void }): T;
declare function withProperty<T>(value: T, handlers: { handle: (e: T) => void }): T;

// Parameter positions are contravariant candidates, combined by their
// common subtype.
const o1 = on((e: Base) => {}, (e: Derived) => {});
const o1b: string = o1.b;
const o2 = on((e: Derived) => {}, (e: Base) => {});
const o2b: string = o2.b;

// A covariant inference assignable to a contravariant candidate is preferred.
const e1 = emit(derived, (e: Base) => {});
const e1b: string = e1.b;
const t1 = tag('', (x: string) => '', (x: Object) => '');
const t1s: string = t1;
const p1 = withProperty(derived, { handle: (e: Base) => {} });
const p1b: string = p1.b;

// Otherwise the contravariant candidate stands.
const e2 = emit(base, (e: Derived) => {});

// A method's parameters are related bivariantly, so they infer as ordinary
// candidates and join the common supertype.
const m1 = withMethod(derived, { handle(e: Base) {} });
const m1b: string = m1.b;
