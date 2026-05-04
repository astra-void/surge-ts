export {};
// TS2322, TS2588, TS2741, TS2451
const const_val = 1;
const_val = 2; // TS2588

let string_val: string = 1; // TS2322

interface Person { name: string; age: number; }
let p: Person = { name: "Alice" }; // TS2741

let redeclared = 1; // TS2451
let redeclared = 2;
