// @filename: setup.ts
export const initialized: boolean = true;
// @filename: index.ts
import "./setup";
let initializedCopy: boolean = initialized;
