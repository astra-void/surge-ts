// @filename: box.ts
interface Box<T> { value: T; } export { Box as Alias };
// @filename: index.ts
import { Alias } from "./box"; let box: Alias<string> = { value: "ok" };
