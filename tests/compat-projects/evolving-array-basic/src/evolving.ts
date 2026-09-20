declare const flag: boolean;
declare const names: string[];
declare const maybe: string[] | undefined;

export function pushed() {
  const out = [];
  out.push(1);
  out.push(2);
  const check: string = out;
  return out;
}

export function looped() {
  const out = [];
  for (const name of names) {
    if (out.length > 0) {
      const last = out[out.length - 1];
    }
    out.push(name);
  }
  const check: number = out;
}

export function indexed() {
  const out = [];
  out[0] = true;
  const check: string = out;
}

export function readBeforePush() {
  const out = [];
  const early = out.slice();
  out.push(1);
}

export function closureOverConst() {
  const out = [];
  out.push(1);
  return () => out;
}

export function closureOverLet() {
  let out = [];
  out.push("a");
  const read = () => out;
  const check: number = read();
}

export function reassigned() {
  let out = [];
  out = [1];
  out.push(2);
  out.push("x");
}

export function neverArrays() {
  const either = flag ? [] : [1];
  const fallback = maybe || [];
  const nested = [[], [1]];
  const bad: number = [];
  const a: string = either;
  const b: number = fallback;
  const c: string = nested;
}
