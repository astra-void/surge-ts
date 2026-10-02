// @filename: user.ts
export const version: number = 1;
// @filename: index.ts
export { Foo } from "./user";
// @filename: app.ts
import { Foo } from "./index";
let value: Foo = 123;
