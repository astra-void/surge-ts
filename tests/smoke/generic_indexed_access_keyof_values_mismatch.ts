type Values<T> = T[keyof T];
type Value = Values<{ name: string; age: number }>;

let bad: Value = true; // TS2322
