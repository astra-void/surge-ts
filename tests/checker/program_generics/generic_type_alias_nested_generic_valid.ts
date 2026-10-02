// @filename: box.ts
type Box<T> = { value: T }; type Outer<T> = { inner: Box<T> };
// @filename: index.ts
let outer: Outer<string> = { inner: { value: "ok" } };
