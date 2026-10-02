// @filename: box.ts
type Named<T extends Missing> = { name: T };
// @filename: index.ts
let value: Named<string> = { name: "ok" };
