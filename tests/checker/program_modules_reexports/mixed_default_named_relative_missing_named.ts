// @filename: thing.ts
export default function makeThing(): string { return "Ada"; }
// @filename: index.ts
import DefaultThing, { helper } from "./thing";
let name: string = DefaultThing();
