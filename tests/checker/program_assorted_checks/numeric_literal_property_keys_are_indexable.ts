// A numeric-literal property key keeps its member: dropping one collapsed the
// whole type literal to `unknown`, which then rejected the index
// (Prisma's generated `Not<B> = { 0: 1; 1: 0 }[B]`).
type Not<B extends 0 | 1> = { 0: 1; 1: 0 }[B];
declare const flipped: Not<0>;
const one: 1 = flipped;
