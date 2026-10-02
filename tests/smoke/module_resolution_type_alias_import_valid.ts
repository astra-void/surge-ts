// @filename: src/a.ts
export type UserId = string;
// @filename: src/b.ts
import type { UserId } from "./a"; let id: UserId = "u1";
