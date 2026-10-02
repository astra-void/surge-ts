// @filename: node_modules/dep/index.d.ts
type Large<T> = { a: T; b: T; c: T; d: T; e: T; f: T }; declare const first: Large<string>; declare const second: Large<string>; export { first, second };
// @filename: src/index.ts
import { first, second } from 'dep'; void first; void second;
