// @filename: box.ts
type Box<T> = { value: T };
// @filename: index.ts
let box: Box = { value: "ok" }; let value = box.value;
