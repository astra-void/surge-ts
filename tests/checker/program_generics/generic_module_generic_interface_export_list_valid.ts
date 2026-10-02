// @filename: box.ts
interface Box<T> { value: T; } export { Box };
// @filename: index.ts
import { Box } from "./box"; let box: Box<string> = { value: "ok" };
