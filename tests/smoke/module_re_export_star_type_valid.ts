// @filename: user.ts
export interface User { name: string; }
export function getName(): string { return "Ada"; }
export const version: string = "1";
// @filename: index.ts
export * from "./user";
// @filename: app.ts
import { User, getName, version } from "./index";
let user: User = { name: getName() + version };
