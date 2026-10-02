// @filename: box.ts
type Box<T = Missing> = { value: T };
// @filename: index.ts
let box: Box = { value: "ok" };
