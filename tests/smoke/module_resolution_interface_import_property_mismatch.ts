// @filename: src/user.ts
export interface User { name: string; }
// @filename: src/index.ts
import { User } from "./user"; let user: User = { name: 123 };
