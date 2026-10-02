// @filename: src/thing.ts
export default function makeThing(): string { return "Ada"; }
export function helper(): number { return 1; }
// @filename: src/index.ts
import DefaultThing, { helper } from "./thing";
let name: string = DefaultThing();
let count: number = helper();
