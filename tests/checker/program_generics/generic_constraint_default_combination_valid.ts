// @filename: box.ts
type Named<T extends string = "ok"> = { name: T };
// @filename: index.ts
let value: Named = { name: "ok" };
