// @filename: user.ts
export interface User { name: string; }
// @filename: index.ts
export type { User } from "./user";
// @filename: app.ts
import { User } from "./index";
let value = User;
