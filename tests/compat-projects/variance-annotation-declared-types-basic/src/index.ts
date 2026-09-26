export {};

interface Shape<V> {
  value: V;
}
class Holder<V> {
  held!: V;
}
type Same<V> = V;
type Wrapped<V> = { wrapped: V };
type Picked<A extends { x: unknown }> = A["x"];
type Chained<V> = Same<Shape<V>>;

type Keys<in T> = keyof T;
type Lookup<out T, out K extends keyof T> = T[K];
type Values<in out T> = T[keyof T];
type Branch<out T> = T extends string ? 1 : 2;
type ShapeOf<in out T> = Shape<T>;
type HolderOf<out T> = Holder<T>;
type SameOf<in out T> = Same<T>;
type PickedOf<out T extends { x: unknown }> = Picked<T>;
type ChainedOf<out T> = Chained<T>;

type Resolved<out T> = { x: { y: T } }["x"];
type WrappedOf<out T> = Wrapped<T>;
type SameObject<out T> = Same<{ value: T }>;
type NeverDropped<out T> = { value: T } | never;

class Members {
  in first = 0;
  out second = 0;
}
declare function annotated<in T>(value: T): void;
