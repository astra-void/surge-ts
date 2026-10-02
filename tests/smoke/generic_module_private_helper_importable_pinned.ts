// @filename: box.ts
type Internal<T> = { value: T }; export type Box<T> = Internal<T>;
// @filename: index.ts
import { Internal } from "./box";
