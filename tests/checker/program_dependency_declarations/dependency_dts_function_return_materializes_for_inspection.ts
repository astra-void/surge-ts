// @filename: node_modules/dep/index.d.ts
export interface Output { value: string } export declare function create(): Output;
// @filename: src/index.ts
import { create } from 'dep'; const value: string = create().value;
