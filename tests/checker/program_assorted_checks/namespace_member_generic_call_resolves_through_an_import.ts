// @filename: util.ts
export namespace util {
  export function first<T>(items: T[]): T {
    return items[0];
  }
}
// @filename: consumer.ts
import { util } from "./util.js";
export const bad: 1 = util.first(["a"]);
