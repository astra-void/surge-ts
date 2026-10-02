// @filename: foo.ts
export const version: number = 1;
// @filename: index.ts
export { version } from "./foo";
// @filename: app.ts
import { version } from "./index";
let value: number = version;
