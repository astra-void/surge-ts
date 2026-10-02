// @filename: node_modules/dep/index.d.ts
interface Item { value: string } declare const item: Item; export { item };
// @filename: src/index.ts
import { item } from 'dep'; const value: string = item.value;
