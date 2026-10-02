// @surge-compare: spans
// @filename: user.ts
export interface User { name: string; }
// @filename: index.ts
import { User as LocalUser } from "./user"; let user: LocalUser = { name: "Ada" };
