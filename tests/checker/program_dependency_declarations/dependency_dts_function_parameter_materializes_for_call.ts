// @filename: node_modules/dep/index.d.ts
export interface Input { value: string } export declare function consume(input: Input): void;
// @filename: src/index.ts
import { consume } from 'dep'; consume({ value: 1 });
