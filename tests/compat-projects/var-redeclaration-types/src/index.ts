type B1 = -1 | 0 | 1;
type B2 = 0 | 1 | -1;
type B3 = 1 | -1 | 0;

export function permutedUnions() {
  var b: B1 = -1;
  var b: B2 = 0;
  var b: B3 = 1;
  return b;
}

export function narrowedFirstDeclaration() {
  var v: string | number = "a";
  var v: string | number;
  var q: string | number = "a";
  var q: string;
  return [v, q];
}

export function annotatedPairs() {
  var w: number[];
  var w: string[];
  var i = 0;
  var i: string;
  return [w, i];
}

export function tupleRest(v: [number, string, boolean]) {
  const [, ...tail] = v;
  const [, , ...last] = v;
  const [, , , ...none] = v;
  const wrongTail: [string] = tail;
  const wrongLast: [string] = last;
  const wrongNone: [string] = none;
  return [wrongTail, wrongLast, wrongNone];
}

export function usingDeclarations() {
  {
    using d = { [Symbol.dispose]() {} };
  }
  {
    using d = { [Symbol.dispose]() {}, value: 1 };
  }
}
