// @filename: foo.ts
export const version: number = 1;
// @filename: index.ts
export { Foo } from "./foo";
// @filename: app.ts
import { Foo } from "./index";
let value: Foo = 123;
