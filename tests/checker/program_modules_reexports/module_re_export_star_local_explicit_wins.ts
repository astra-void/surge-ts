// @filename: other.ts
export const greeting: number = 1;
// @filename: index.ts
export const greeting: string = "Ada";
export * from "./other";
// @filename: app.ts
import { greeting } from "./index";
let value: string = greeting;
