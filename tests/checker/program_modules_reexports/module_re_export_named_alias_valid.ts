// @filename: foo.ts
export interface Foo { name: string; }
// @filename: index.ts
export { Foo as FooModel } from "./foo";
// @filename: app.ts
import { FooModel } from "./index";
let value: FooModel = { name: "Ada" };
