// @filename: user.ts
export interface User { name: string; }
// @filename: index.ts
import { User as UserModel } from "./user";
let user: UserModel = { name: "Ada" };
