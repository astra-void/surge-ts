// The same program under `strictNullChecks`.
declare let maybe: number | undefined;
declare let opaque: unknown;
export const a: number = maybe;
export const b = maybe.toFixed();
export const c: string = undefined;
let d = undefined;
d = 1;
export const e = opaque.name;
export const f = opaque();
