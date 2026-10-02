// @filename: src/a.ts
export interface User { name: string; }
// @filename: src/b.ts
import { Missing } from "./a"; let value: Missing = "x";
