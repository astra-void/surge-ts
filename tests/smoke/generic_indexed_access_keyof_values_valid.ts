type Values<T> = T[keyof T];
type Value = Values<{ name: string; age: number }>;

let a: Value = "Ada";
let b: Value = 42;
