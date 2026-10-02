// @filename: other.ts
export const label: number = 1;
// @filename: index.ts
export const label: string = "Ada";
export * from "./other";
// @filename: app.ts
import { label } from "./index";
let value: string = label;
