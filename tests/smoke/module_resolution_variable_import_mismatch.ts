// @filename: src/a.ts
export const version: string = "1";
// @filename: src/b.ts
import { version } from "./a"; let current: number = version;
