// @filename: box.ts
type Box<T> = { value: T }; export { Box };
// @filename: index.ts
import { Box } from "./box"; let box: Box<string> = { value: "ok" };
