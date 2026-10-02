// @filename: user.ts
export function getName(): string { return "Ada"; }
// @filename: a.ts
import { getName } from "./user";
let value: string = getName();
