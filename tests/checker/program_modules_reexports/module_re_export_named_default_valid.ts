// @filename: foo.ts
export default function getName(): string { return "Ada"; }
// @filename: index.ts
export { default as DefaultThing } from "./foo";
// @filename: app.ts
import { DefaultThing } from "./index";
let name: string = DefaultThing();
