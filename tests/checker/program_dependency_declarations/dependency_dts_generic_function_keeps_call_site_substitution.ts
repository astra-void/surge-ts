// @filename: node_modules/dep/index.d.ts
export declare function identity<T extends { value: string } = { value: string }>(input: T): T;
// @filename: src/index.ts
import { identity } from 'dep'; const value: string = identity({ value: 'ok' }).value;
