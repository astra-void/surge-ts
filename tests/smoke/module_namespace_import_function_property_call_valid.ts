// @filename: user.ts
export function getName(): string { return "Ada"; }
export const version: string = "1";
// @filename: index.ts
import * as user from "./user";
let name: string = user.getName();
let version: string = user.version;
