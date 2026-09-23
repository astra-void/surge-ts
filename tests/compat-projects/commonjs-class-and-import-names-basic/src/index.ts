export class Object {}
declare class Other {}
namespace N {
  export type T = string;
  export const v = 1;
  export interface I {}
  export namespace Inner { export const q = 1; }
}
import string = N.T;
import number = N.v;
import boolean = N.I;
import bigint = N.Inner;
import ok = N.T;
