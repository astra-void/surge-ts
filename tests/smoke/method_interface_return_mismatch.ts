interface Calc {
  add(a: number, b: number): number;
}

declare const calc: Calc;

const result: string = calc.add(1, 2);
