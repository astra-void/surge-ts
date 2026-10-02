interface E<T> { data: T } declare function isE<T>(v: unknown): v is E<T>; export function f(x: { a: 1 } | E<string>) { if (isE<string>(x)) { const n: number = x.data; } }
