// @filename: node_modules/dep/index.d.cts
type Item = { value: string }; declare const item: Item; export { item };
// @filename: src/index.ts
import { item } from 'dep'; const value: string = item.value;
// @filename: node_modules/dep/package.json
{ "name": "dep", "types": "index.d.cts" }
