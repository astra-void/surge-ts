// @filename: box.ts
type Box<T> = { value: T }; function takesString(value: string): void { }
// @filename: index.ts
let box: Box = { value: "ok" }; takesString(box.value);
