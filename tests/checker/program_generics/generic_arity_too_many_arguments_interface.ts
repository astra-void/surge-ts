// @filename: box.ts
interface Box<T> { value: T; }
// @filename: index.ts
let box: Box<string, number> = { value: "ok" };
