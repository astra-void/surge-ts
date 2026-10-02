// @filename: box.ts
type Box<T> = { value: T }; type Alias<T> = Box<T>;
// @filename: index.ts
let box: Alias<string> = { value: "ok" };
