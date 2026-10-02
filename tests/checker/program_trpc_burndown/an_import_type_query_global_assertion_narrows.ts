// @types: chai, dep/globals
// @filename: node_modules/chai/index.d.ts
declare namespace Chai { interface Assert { (expression: any, message?: string): asserts expression; } }
// @filename: node_modules/dep/index.d.ts
declare const assert: Chai.Assert; export { assert };
// @filename: node_modules/dep/globals.d.ts
declare global { let assert: typeof import('dep')['assert']; } export {};
// @filename: src/index.ts
interface E { data: number } declare function isE(v: unknown): v is E; export function f(err: unknown) { assert(isE(err)); const s: string = err.data; }
