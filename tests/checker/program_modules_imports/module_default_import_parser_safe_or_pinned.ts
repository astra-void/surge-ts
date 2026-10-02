// @filename: user.ts
export default function getName(): string { return "Ada"; }
// @filename: index.ts
import DefaultThing from "./user";
let name: string = DefaultThing();
