interface Calc {
  add(a: Missing, b: number): number;
}

declare const calc: Calc;

const result: number = calc.add(1, 2);
