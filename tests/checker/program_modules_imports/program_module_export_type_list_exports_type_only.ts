// @filename: a.ts
type User = { name: string };
export type { User };
// @filename: b.ts
import type { User } from "./a";
let user: User = { name: "Ada" };
let value = User;
