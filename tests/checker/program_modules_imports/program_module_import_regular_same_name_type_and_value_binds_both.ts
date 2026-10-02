// @filename: user.ts
export interface User { name: string; }
export const User: string = "Ada";
// @filename: index.ts
import { User } from "./user";
let user: User = { name: "Ada" };
let value: string = User;
