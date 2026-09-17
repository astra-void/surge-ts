export {}
declare const opt: { a?: number; b: number };
declare const ro: { readonly k?: number };
declare const idx: { [k: string]: number };
declare const anyo: any;
declare const nested: { o: { a?: number } };
declare let v: number;

delete opt.a;
delete nested.o.a;
delete idx["x"];
delete anyo.whatever;

delete opt.b;
delete ro.k;
delete v;

const asBoolean: boolean = delete opt.a;
const notNumber: number = delete opt.a;

declare const guarded: { a?: number; deep: { a?: number } };

if (guarded.a) {
  delete guarded.a;
}
if (guarded.deep.a) {
  delete guarded.deep.a;
}
