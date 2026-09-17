export {}
enum BackwardIsFine { X = 1, Y = X }
enum AutoValues { P, Q }
enum StringBackward { S = "s", T = S }
enum ComputedBackward { A = 1 << 2, B = A | 1 }

enum ForwardPlain { A = B, B = 1 }
enum ForwardInExpression { M = N + 1, N = 2 }
const enum ForwardInConstEnum { A = B, B = 1 }
