// @filename: foo.ts
export const value: number = 1;
// @filename: index.ts
export * as Foo from "./foo";
// @filename: app.ts
import { Foo } from "./index";
let value: number = Foo.value;
