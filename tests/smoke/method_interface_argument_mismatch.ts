interface Calc {
  add(a: number, b: number): number;
}

declare const calc: Calc;

const result: number = calc.add(1, "two");
