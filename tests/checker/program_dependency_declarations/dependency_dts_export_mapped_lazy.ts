// @filename: node_modules/dep/index.d.ts
type Copy<T> = { [K in keyof T]: T[K] }; declare const item: Copy<{ value: string }>; export { item };
// @filename: src/index.ts
import { item } from 'dep'; const value: string = item.value;
