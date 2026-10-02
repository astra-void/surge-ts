// @filename: foo.ts
export function getName(): string { return "Ada"; }
// @filename: index.ts
export { getName } from "./foo";
// @filename: app.ts
import { getName } from "./index";
let name: string = getName();
