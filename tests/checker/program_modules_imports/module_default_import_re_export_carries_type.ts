// @filename: dispatcher.ts
declare class Dispatcher { id: string }
export default Dispatcher;
// @filename: mid.ts
import Dispatcher from "./dispatcher";
export { Dispatcher };
// @filename: index.ts
import type { Dispatcher } from "./mid";
let value: Dispatcher = 1 as any;
