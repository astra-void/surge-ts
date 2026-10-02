// @filename: src/a.ts
export interface User { name: string; }
// @filename: src/b.ts
import { User as UserModel } from "./a"; let user: UserModel = { name: "Ada" };
