// @filename: user.ts
export interface User { name: string; }
export function getName(): string { return "Ada"; }
// @filename: index.ts
export { User, getName } from "./user";
// @filename: app.ts
import { User, getName } from "./index";
let user: User = { name: getName() };
