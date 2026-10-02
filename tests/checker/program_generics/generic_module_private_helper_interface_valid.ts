// @filename: box.ts
type Internal<T> = { value: T }; export interface Box<T> { value: Internal<T>; }
// @filename: index.ts
import { Box } from "./box"; let box: Box<string> = { value: { value: "ok" } };
