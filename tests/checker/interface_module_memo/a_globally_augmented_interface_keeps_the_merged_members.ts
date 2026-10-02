// @filename: globals.d.ts
// An interface that gains members from a later `declare global` augmentation
// must not be served an expansion memoized before the merge. The memo key
// carries the live declaration table's version for exactly this reason; without
// it, `NodeJS.ProcessEnv` — whose index signature arrives through
// `extends Dict<string>` in a second declaration — lost that signature and
// every `process.env.FOO` became a TS2339.
interface Dict<T> { [key: string]: T | undefined }
interface Env { HOME: string }
// @filename: augment.ts
declare global {
interface Env extends Dict<string> { PATH: string }
}
export {};
// @filename: consumer.ts
declare const env: Env;
export const home = env.HOME;
export const path = env.PATH;
export const anything = env.ANYTHING_AT_ALL;
