declare function takesArray(x: string[]): void;
declare function takesPair(x: [string, string]): void;
declare function takesReadonly(x: readonly string[]): void;
declare const ra: readonly string[];
declare const rt: readonly [string, string];
declare const rn: readonly number[];
declare let ma: string[];
declare let mt: [string, string];
declare let maybe: string[] | undefined;
declare let either: string[] | number;
type Mutable = string[];
type Frozen = readonly string[];
declare let orMissing: Mutable | undefined;

// TS4104 replaces the head, for an argument as anywhere else.
takesArray(ra);
takesArray(rt);
takesPair(ra);
takesPair(rt);
const v1: string[] = ra;
const v2: [string, string] = rt;
ma = ra;
ma = rt;
mt = ra;
mt = rt;
ma = rn;
maybe = ra;
orMissing = rt;
function ret(): string[] {
  return ra;
}
const holder: { a: string[] } = { a: ra };
class Holder {
  p: string[] = ra;
}
let target: { a: string[] } = { a: [] };
target.a = ra;

// No single mutable array target: the head stays.
either = ra;
declare let readonlyPair: readonly [string, string];
readonlyPair = ra;
declare let lengthAsString: { length: string };
lengthAsString = ra;

// Accepted.
takesReadonly(ma);
const ok1: readonly string[] = ma;
const ok2: readonly string[] = rt;
const ok3: Frozen = ra;
const ok4: { length: number } = ra;

export { v1, v2, ret, holder, Holder, target, ok1, ok2, ok3, ok4 };
