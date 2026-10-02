// @filename: box.ts
type Box<T extends Missing> = { value: T };
// @filename: index.ts
let box: Box<string> = { value: "ok" };
