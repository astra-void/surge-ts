// @filename: box.ts
type Named<T extends string> = { name: T };
// @filename: index.ts
let value: Named<number> = { name: 1 };
