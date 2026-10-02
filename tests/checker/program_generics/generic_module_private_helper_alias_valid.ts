// @filename: box.ts
type Internal<T> = { value: T }; export type Box<T> = Internal<T>;
// @filename: index.ts
import { Box } from "./box"; let box: Box<string> = { value: "ok" };
