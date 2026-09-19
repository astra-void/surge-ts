export {};
interface A { a: number; b: string }
interface B { a: number }
interface Wide { a: number; b?: string; c: number; d: number }
declare let b: B;
let x1: A = b;
let x2: A | undefined = b;
let x3: Wide = b;
function f(p: A) {}
f(b);
let x4: A = { a: 1 };
class K { a = 1; }
let x5: A = new K();
let x6: { a: number } = 1 as number;
let x7: (() => void) & { z: number } = () => {};
declare let o: object;
let x8: { foo: string } = o;
interface Box<T> { value: T }
declare let bb: Box<B>;
let x9: Box<A> = bb;
