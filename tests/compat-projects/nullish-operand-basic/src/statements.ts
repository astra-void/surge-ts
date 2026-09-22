enum Kind { A, B }
declare let u: number | undefined;
declare let o: object | undefined;
declare let f: (() => void) | undefined;
declare let g: () => number;

for (const k in null) {}
for (const k in u) {}
for (const k in (null)) {}
for (const k in o) {}
for (const k in Kind) {}
for (const k in [1, 2]) {}

null();
undefined();
f();
(f)();

export const constructed = new f();
export const fromCall = new g();

export function body(p: number | undefined, callback?: () => void): number {
  for (const k in p) {}
  callback();
  if (p !== undefined) {
    return p * 2;
  }
  return (p) + 1;
}
