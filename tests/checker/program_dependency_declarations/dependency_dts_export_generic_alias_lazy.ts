// @filename: node_modules/dep/index.d.ts
type Box<T> = { value: T }; declare const item: Box<string>; export { item };
// @filename: src/index.ts
import { item } from 'dep'; const value: string = item.value;
