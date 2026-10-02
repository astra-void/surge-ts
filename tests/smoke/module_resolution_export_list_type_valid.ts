// @filename: src/a.ts
type UserId = string; export type { UserId };
// @filename: src/b.ts
import type { UserId } from "./a"; let id: UserId = "u1";
