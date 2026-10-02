// @filename: node_modules/dep/index.d.ts
type Item = { value: string }; declare const item: Item; export { item };
// @filename: src/index.ts
import { item } from 'dep'; const a: { value: string } = item; const b: { value: string } = item;
