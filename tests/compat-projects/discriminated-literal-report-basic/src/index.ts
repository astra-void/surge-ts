type ADT =
  | { tag: "A"; a1: string }
  | { tag: "D"; d20: number }
  | { tag: "T" };

declare let wrong: ADT;

export function reports(): void {
  wrong = { tag: "T", a1: "extra" };
  wrong = { tag: "A", d20: 12 };
  wrong = { tag: "D" };
  wrong = { tag: "A", a1: "ok" };
}

type Ambiguous =
  | { tag: "A"; x: string }
  | { tag: "A"; y: number }
  | { tag: "B"; z: boolean };

declare let amb: Ambiguous;

export function ambiguous(): void {
  amb = { tag: "A", x: "hi" };
  amb = { tag: "A", y: 12 };
  amb = { tag: "A" };
}
