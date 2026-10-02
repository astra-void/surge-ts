// @filename: a.ts
const value: string = "Ada";
export { value };
// @filename: b.ts
import { value } from "./a";
let copy: string = value;
