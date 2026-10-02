// @filename: user.ts
export interface User { name: string; }
// @filename: a.ts
import { User } from "./user";
let user: User = { name: "Ada" };
