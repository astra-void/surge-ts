// @filename: foo.ts
export function makeThing(): string { return "Ada"; }
// @filename: index.ts
export * from "./foo";
// @filename: app.ts
import { makeThing } from "./index";
let value: string = makeThing();
