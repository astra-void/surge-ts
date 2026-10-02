// @filename: node_modules/dep/index.d.ts
interface Left { left: string } interface Right { right: number } declare const item: Left & Right; export { item };
// @filename: src/index.ts
import { item } from 'dep'; const left: string = item.left; const right: number = item.right;
