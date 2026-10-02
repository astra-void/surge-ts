// @filename: dispatcher.ts
declare class Dispatcher { id: string }
export default Dispatcher;
// @filename: index.ts
import Dispatcher from "./dispatcher";
let value: Dispatcher = 1 as any;
