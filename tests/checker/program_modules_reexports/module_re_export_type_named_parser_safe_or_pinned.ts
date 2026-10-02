// @filename: foo.ts
export interface Foo { name: string; }
// @filename: index.ts
export type { Foo } from "./foo";
// @filename: app.ts
import type { Foo } from "./index";
let value: Foo = { name: "Ada" };
