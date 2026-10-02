// @filename: box.ts
type Box<T> = { value: T }; function takesString(value: string): void { }
// @filename: index.ts
let box: Box<Missing> = { value: 123 }; takesString(box.value);
