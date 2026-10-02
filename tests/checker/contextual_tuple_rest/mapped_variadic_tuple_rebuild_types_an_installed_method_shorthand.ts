// The zod `_LazyMethodsOf<T>` shape: a mapped type rebuilds every method as
// `(this: T, ...args: A) => R` with `A` an inferred tuple, and the installed
// object literal's method shorthands are contextually typed by it.
// @noImplicitAny: true
type Methods<T> = Partial<{
[K in keyof T]: T[K] extends (...a: infer A) => infer R ? (...a: A) => R : never;
}>;
declare function install<T extends object>(instance: T, methods: Methods<T>): void;
interface Shape { gt(value: number, message?: string): Shape }
declare const shape: Shape;
install(shape, {
gt(value, message) {
void value;
void message;
return shape;
},
});
