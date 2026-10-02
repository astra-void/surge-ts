// @filename: src/a.ts
export interface User { name: string; }
// @filename: src/b.ts
import type { User } from "./a"; let user: User = { name: "Ada" };
