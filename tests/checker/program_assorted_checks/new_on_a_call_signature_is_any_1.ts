// TS18048 on the possibly-`undefined` target; without `noImplicitAny` a
// non-`void` call signature is TS2350 rather than TS7009.
declare let f: (() => void) | undefined;
declare let g: () => number;
export const a = new f();
export const b = new g();
