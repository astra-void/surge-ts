// @filename: user.ts
export function getName(): string { return "Ada"; }
// @filename: index.ts
import * as user from "./user";
let value = user.missing;
