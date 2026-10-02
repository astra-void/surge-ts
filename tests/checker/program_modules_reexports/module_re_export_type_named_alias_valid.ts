// @filename: foo.ts
export interface Foo { name: string; }
// @filename: index.ts
export type { Foo as FooModel } from "./foo";
// @filename: app.ts
import type { FooModel } from "./index";
let value: FooModel = { name: "Ada" };
