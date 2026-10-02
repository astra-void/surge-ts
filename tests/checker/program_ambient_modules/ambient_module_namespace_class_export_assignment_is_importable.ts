// @filename: ambient.d.ts
declare module "mystream" {
  namespace Stream {
    class Readable { ended: boolean }
  }
  export = Stream;
}
// @filename: index.ts
import type { Readable } from "mystream";
let value: Readable = 1 as any;
