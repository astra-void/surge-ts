// @filename: box.ts
type Box<T = string> = { value: T };
// @filename: index.ts
let box: Box = { value: "ok" };
