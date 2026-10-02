// @filename: node_modules/dep/primitive.d.ts
declare const primitive: { value: string }; export { primitive };
// @filename: node_modules/dep/index.d.ts
import { primitive } from './primitive'; declare const item: typeof primitive; export { item };
// @filename: src/index.ts
import { item } from 'dep'; const value: string = item.value;
