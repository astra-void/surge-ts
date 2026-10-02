// @filename: user.ts
export type UserId = string;
// @filename: a.ts
import type { UserId } from "./user";
let id: UserId = "u1";
