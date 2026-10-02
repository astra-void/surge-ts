// @noImplicitAny: true
declare let f: (() => void) | undefined;
declare let g: () => number;
export const a = new f();
export const b = new g();
