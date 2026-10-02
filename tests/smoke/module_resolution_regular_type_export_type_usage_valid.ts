// @filename: user.ts
export interface User { name: string; }
// @filename: index.ts
import { User } from "./user";
let user: User = { name: "Ada" };
