// @filename: box.ts
export function identity<T>(value: T): T { return value; }
// @filename: index.ts
import { identity } from "./box"; let value = identity("ok");
