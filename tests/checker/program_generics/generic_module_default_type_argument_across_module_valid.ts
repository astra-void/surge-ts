// @filename: box.ts
export type Box<T = string> = { value: T };
// @filename: index.ts
import { Box } from "./box"; let box: Box = { value: "ok" };
