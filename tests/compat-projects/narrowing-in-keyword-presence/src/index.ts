export {};
function positiveIntersection(x: { a: string } & { b: string }) {
  if ("a" in x) {
    let s: string = x.a;
  } else {
    let n: never = x;
  }
}

function narrowsToNever(x: { l: number } | { r: number }) {
  let v: number;
  if ("l" in x) {
    v = x.l;
  } else if ("r" in x) {
    v = x.r;
  } else {
    v = x;
  }
  return v;
}

type AOrB = { aProp: number } | { bProp: number };
function chain(x: AOrB) {
  if ("aProp" in x) {
    x.aProp;
  } else if ("bProp" in x) {
    x.bProp;
  } else if ("cProp" in x) {
    const n: never = x;
  }
}

class Unreachable {
  a: string = "";
  inThis() {
    if ("a" in this) {
    } else {
      let y = this.a;
    }
  }
}

function unknownKey(x: { a: number } | { b: number }) {
  if ("c" in x) {
    const c: unknown = x.c;
  } else {
    const y: { a: number } | { b: number } = x;
  }
}

function optionalKey(x: { a?: number } | { b: number }) {
  if ("a" in x) {
    const y: { a?: number } = x;
  } else {
    const y: { a?: number } | { b: number } = x;
  }
}

function indexed(x: { [key: string]: number } | { b: string }) {
  if ("a" in x) {
    const y: { [key: string]: number } = x;
  } else {
    const z: { [key: string]: number } | { b: string } = x;
  }
}

function single(x: { a: string }) {
  if (!("a" in x)) {
    const n: never = x;
  }
  const s = "a" in x ? x.a : 0;
}

function numericKey(x: object, y: { 1: string } | { 2: number }) {
  if (1 in x) {
    x[1];
    x["1"];
  }
  if (1 in y) {
    const s: string = y[1];
  } else {
    const n: number = y[2];
  }
}
