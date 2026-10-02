// @noImplicitAny: true
// @filename: src/index.ts
// A namespace member's generic interface (`Ev<T>`) must shadow a same-named
// non-generic ambient global (`interface Ev`) when referenced from a sibling
// member. Otherwise a handler-alias chain (`Handler<T> = Fn<Ev<T>>`) resolves
// `Ev` to the arity-0 global, applying `<T>` degrades it to a non-function, and
// a callback contextually typed by `Handler` falsely reports TS7006 — the root
// cause of the React `onClick={(e) => …}` / `render={({ field }) => …}` over-reports.
declare interface Ev { a: number; }
declare namespace NS {
interface Base<T> { x: T }
interface Ev<T> extends Base<T> { y: number }
type Fn<E extends Base<any>> = (e: E) => void;
type Handler<T> = Fn<Ev<T>>;
}
declare function on(cb: NS.Handler<number>): void;
on((e) => e.x);
