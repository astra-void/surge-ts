// @filename: user.ts
export function getName(): string { return "Ada"; }
// @filename: index.ts
import * as ns from "./user";
let value = getName;
