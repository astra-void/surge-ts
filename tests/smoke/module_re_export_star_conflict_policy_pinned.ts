// @filename: a.ts
export const label: string = "Ada";
// @filename: b.ts
export const label: number = 1;
// @filename: index.ts
export * from "./a";
export * from "./b";
// @filename: app.ts
import { label } from "./index";
let value: string = label;
