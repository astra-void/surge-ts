import type { createThing, CONST_V } from './leaf';
import type { createThing as c2, CONST_V as v2 } from './mid';

export type A = typeof createThing;
export type B = typeof CONST_V;
export type C = typeof c2;
export type D = typeof v2;
