// @filename: user.ts
export default function getName(): string { return "Ada"; }
export const version: number = 1;
// @filename: index.ts
export * from "./user";
// @filename: app.ts
import getName from "./index";
let value = getName;
