type T = { a: number, b: string };
var v01: T;
var v01: { [P in keyof T]: T[P] };
var v01: Pick<T, keyof T>;
interface Point { x: number; y: number }
var p: Point;
var p: { x: number; y: number };
var u: string | boolean;
var u: boolean | string;
interface A { a: string } interface B { b: string }
var i: A & B;
var i: B & A;
var fl: () => number;
var fl: { (): number };
var bad: string;
var bad: number;
var opt: { a?: string };
var opt: { a: string };
export {};
