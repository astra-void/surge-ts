declare function run<T, U extends unknown[]>(fn: (...args: U) => T, ...args: U): T;

// The trailing arguments infer `U` as one tuple, and a callback parameter
// written without a type takes that tuple's element at its position.
run((foo) => foo.length, "foo");
run((foo) => foo, undefined);
const sum: number = run((a, b) => a + b, 1, 2);
run((...all) => all.length, "a", 1);

// An optional parameter is an optional element of the tuple it infers.
run((foo: string, bar?: number) => "x", "foo", undefined);
run((foo: string, bar?: number) => "x", "foo");
run((foo: string, bar?: number) => "x", "foo", 42);

run((foo: string) => "x", 42);
run((foo) => foo * 2, "s");
