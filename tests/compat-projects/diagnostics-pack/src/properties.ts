export {};
// TS2339, TS2353
let obj = { a: 1 };
obj.b; // TS2339

interface StrictObj { a: number; }
let s: StrictObj = { a: 1, b: 2 }; // TS2353
