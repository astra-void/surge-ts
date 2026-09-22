export {};

const early = Early.A;
const inlined = LaterConst.B;
const ambient = Ambient.C;
function deferred() {
  return Early.A;
}
enum Early {
  A,
}
const enum LaterConst {
  B,
}
declare enum Ambient {
  C,
}
