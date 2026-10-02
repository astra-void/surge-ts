// @filename: a.ts
interface User { name: string; }
// @filename: setup.ts
export {};
// @filename: b.ts
import "./setup";
let user: User = { name: "Ada" };
