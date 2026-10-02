// @surge-compare: order
// @filename: user.ts
export interface User { name: string; }
// @filename: a.ts
import { Missing, AlsoMissing } from "./user";
let value: Missing = 123;
// @filename: b.ts
let value: number = "bad";
