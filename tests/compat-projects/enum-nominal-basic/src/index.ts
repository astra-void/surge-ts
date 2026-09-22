export {};
enum E { A, B }
enum F { X, Y }
declare let e: E; declare let f: F;
e = f;
f = e;
e = 1;
e = 5;
let member: E.A = E.B;
declare let either: E | F;
let onlyE: E = either;
let fine: E = E.A;
let alsoFine: E.A = E.A;
function takesE(x: E) {}
takesE(F.X);
takesE(E.B);
const n: number = e;
