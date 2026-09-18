const o = { a: 1, b: 2 };
declare const k: keyof typeof o;

export const v = o[k];
