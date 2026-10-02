// @filename: node_modules/dep/index.d.ts
export type Large<T> = { a: T; b: T; c: T; d: T }; export declare function create(input: Large<string>): Large<string>;
// @filename: src/index.ts
import { create } from 'dep'; void create;
