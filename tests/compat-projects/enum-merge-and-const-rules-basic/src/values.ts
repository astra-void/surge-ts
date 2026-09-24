const enum E {
  A = 1 / 0,
  B = -1 / 0,
  C = 0 / 0,
  D = 2 ** 1024,
  F = A + 1,
  G = E.C * 2,
  H = 1,
  I = H / 0,
  J = 3,
}
enum NotConst { A = 1 / 0, B = 0 / 0 }
declare const enum Amb { A = 1 / 0 }
export {};
