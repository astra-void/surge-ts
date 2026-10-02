// @filename: src/index.ts
let id: ID = "ok"; let user: User = { name: "Ada" };
// @filename: types/globals.d.ts
declare type ID = string; declare interface User { name: string; }
