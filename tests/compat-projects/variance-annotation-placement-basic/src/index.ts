export {};

function onFunction<in T>() {}
const onArrow = <out T,>() => {};
type Identity<in out T> = T;
type Either<out T> = T | undefined;
type Consumer<in T> = (value: T) => void;
type Producer<out T> = { value: T };
type Mapped<out T> = { readonly [K in keyof T]: T[K] };
interface Box<out T> {
  value: T;
}
class Sink<in T> {
  put(value: T) {}
}
