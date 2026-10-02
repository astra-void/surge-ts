// @filename: box.ts
export type Box<T> = { value: T };
// @filename: index.ts
import type { Box } from "./box"; let value = Box;
