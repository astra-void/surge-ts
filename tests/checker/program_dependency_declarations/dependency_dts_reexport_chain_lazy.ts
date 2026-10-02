// @filename: node_modules/dep/base.d.ts
interface Item { value: string } declare const item: Item; export { item };
// @filename: node_modules/dep/middle.d.ts
export { item } from './base';
// @filename: node_modules/dep/index.d.ts
export { item } from './middle';
// @filename: src/index.ts
import { item } from 'dep'; const value: string = item.value;
