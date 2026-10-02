// @filename: node_modules/dep/index.d.mts
export declare function consume(input: { value: string }): void;
// @filename: src/index.ts
import { consume } from 'dep'; consume({ value: 1 });
// @filename: node_modules/dep/package.json
{ "name": "dep", "types": "index.d.mts" }
