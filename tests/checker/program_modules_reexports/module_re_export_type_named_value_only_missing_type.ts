// @filename: foo.ts
// `export type { Foo }` over a value-only export is legal and republishes
// the symbol, so the re-export itself is not TS2305. Using the value as a
// type is the error, TS2749.
export const Foo: string = "Ada";
// @filename: index.ts
export type { Foo } from "./foo";
// @filename: app.ts
import type { Foo } from "./index";
let value: Foo = { name: "Ada" };
