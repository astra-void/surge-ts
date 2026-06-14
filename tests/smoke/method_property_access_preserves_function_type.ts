interface Calc {
  add(a: number, b: number): number;
}

declare const calc: Calc;

let fn: (a: number, b: number) => number = calc.add;
