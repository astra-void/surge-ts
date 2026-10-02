// @filename: a.ts
export const greeting: string = "Ada";
// @filename: b.ts
export const greeting: number = 1;
// @filename: index.ts
export * from "./a";
export * from "./b";
// @filename: app.ts
import { greeting } from "./index";
let value: string = greeting;
