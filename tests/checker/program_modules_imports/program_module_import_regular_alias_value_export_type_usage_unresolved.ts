// @filename: user.ts
export const User: string = "Ada";
// @filename: index.ts
import { User as UserModel } from "./user";
let value: UserModel = "Ada";
