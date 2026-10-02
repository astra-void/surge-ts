// @filename: box.ts
type Box<T> = { value: T };
// @filename: index.ts
let box: Box<Missing> = { value: 123 };
