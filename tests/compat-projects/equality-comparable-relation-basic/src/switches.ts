export {};
class Base { a = ""; }
class Derived extends Base { b = ""; }
class NumericA { a = 1; }
declare const base: Base;
declare const derived: Derived;
declare const numericA: NumericA;
declare const ab: "a" | "b";
declare const pair: [string, number];
declare const strings: string[];
declare const text: string;

switch (base) {
  case derived:
  case numericA:
    break;
}
switch (ab) {
  case "a":
  case "c":
    break;
}
switch (strings) {
  case pair:
    break;
}
switch (text) {
  case "x":
  case 1:
    break;
}
