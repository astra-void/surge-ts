// @filename: types.ts
// A memoized expansion must not swallow a diagnostic its body emitted: only
// expansions whose body emitted nothing are stored, so an unresolved member
// type still reports however many consumers instantiate the declaration.
export interface Box<T> { value: T; extra: NotDeclaredAnywhere }
// @filename: consumer.ts
import { Box } from "./types";
declare const a: Box<string>;
declare const b: Box<string>;
declare const c: Box<number>;
export const used = [a.value, b.value, c.value];
