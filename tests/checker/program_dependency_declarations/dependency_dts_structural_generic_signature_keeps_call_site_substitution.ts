// @filename: node_modules/dep/index.d.ts
export declare function transform<T extends string>(input: { value: T }): { output: T };
// @filename: src/index.ts
import { transform } from 'dep'; const output: 'ok' = transform({ value: 'ok' }).output; transform({ value: 1 });
