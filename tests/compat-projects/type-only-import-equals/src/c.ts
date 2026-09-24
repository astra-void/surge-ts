import type A = require('./a');
import type = require('./b');
import type M = require('./m');

A.prototype;
const a: A = { a: 'a' };
const aBad: A = { a: 1 };
void type;
export declare const AConstructor: typeof A;
M.x;
let i: M.I = { i: 1 };
let iBad: M.I = { i: "no" };
