namespace N { export class B {} }
var b1: N.A;
var b2: N.Missing2;
namespace M.P { export type T = number; }
var b3: M.P.T;
var b4: M.P.Q;
declare namespace D { interface Hidden {} export interface Shown {} }
var b5: D.Hidden;
var b6: D.Shown;
