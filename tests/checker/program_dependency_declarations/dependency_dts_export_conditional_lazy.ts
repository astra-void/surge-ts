// @filename: node_modules/dep/index.d.ts
type Select<T> = T extends string ? { text: T } : { count: number }; declare const item: Select<string>; export { item };
// @filename: src/index.ts
import { item } from 'dep'; const value: string = item.text;
