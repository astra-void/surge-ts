// @filename: box.ts
export type Box<T> = { value: T };
// @filename: index.ts
import { Box } from "./box"; let box: Box = { value: "ok" };
