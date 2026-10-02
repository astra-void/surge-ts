// @filename: box.ts
type Box<T> = { value: T }; interface UsesBox<T> { inner: Box<T>; }
// @filename: index.ts
let value: UsesBox<string> = { inner: { value: "ok" } };
