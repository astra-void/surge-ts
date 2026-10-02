// @filename: box.ts
export interface Box<T> { value: T; }
// @filename: index.ts
import { Box } from "./box"; let box: Box<string> = { value: "ok" };
