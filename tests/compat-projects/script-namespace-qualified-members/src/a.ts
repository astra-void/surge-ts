namespace N { export class A {} export namespace Inner { export interface I {} } }
var a1: N.A;
var a2: N.B;
var a3: N.Missing;
var a4: N.Inner.I;
var a5: N.Inner.Missing;
