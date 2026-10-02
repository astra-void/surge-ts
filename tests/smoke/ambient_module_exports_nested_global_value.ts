// @filename: buf.d.ts
declare module "mybuf" {
  global {
    interface MyBuf { length: number }
    var MyBuf: { from(s: string): MyBuf };
  }
  export { MyBuf };
}
declare module "node:mybuf" {
  export * from "mybuf";
}
// @filename: index.ts
import { MyBuf } from "node:mybuf";
export const a: number = MyBuf.from("x").length;
