// @filename: other.ts
export interface User { other: number; }
// @filename: index.ts
export interface User { name: string; }
export * from "./other";
// @filename: app.ts
import { User } from "./index";
let user: User = { name: "Ada" };
