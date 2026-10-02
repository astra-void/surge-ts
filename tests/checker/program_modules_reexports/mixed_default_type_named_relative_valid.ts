// @filename: thing.ts
export default function makeThing(): string { return "Ada"; }
export interface User { name: string; }
// @filename: index.ts
import DefaultThing, { type User } from "./thing";
let user: User = { name: DefaultThing() };
