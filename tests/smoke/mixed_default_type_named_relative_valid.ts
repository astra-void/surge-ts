// @filename: src/thing.ts
export default function makeThing(): string { return "Ada"; }
export interface User { name: string; }
// @filename: src/index.ts
import DefaultThing, { type User } from "./thing";
let user: User = { name: DefaultThing() };
