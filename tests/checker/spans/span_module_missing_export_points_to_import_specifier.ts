// @surge-compare: spans
// @filename: user.ts
export interface User { name: string; }
// @filename: index.ts
import { Missing } from "./user"; let value: Missing = "x";
