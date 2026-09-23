export {};
function twoLevels(x: string | number | boolean) {
  const isString = typeof x === "string";
  const isNumber = typeof x === "number";
  const isStringOrNumber = isString || isNumber;
  if (isStringOrNumber) {
    const t: string | number = x;
    const bad: string = x;
  } else {
    const t: boolean = x;
  }
}
function fiveLevels(x: string | number) {
  const a1 = typeof x === "string";
  const a2 = a1;
  const a3 = a2;
  const a4 = a3;
  const a5 = a4;
  if (a5) {
    const t: string = x;
  }
}
function sixLevels(x: string | number) {
  const a1 = typeof x === "string";
  const a2 = a1;
  const a3 = a2;
  const a4 = a3;
  const a5 = a4;
  const a6 = a5;
  if (a6) {
    const t: string = x;
  }
}
function inExpressions(x: string | number | boolean) {
  const isString = typeof x === "string";
  const isNumber = typeof x === "number";
  const either = isString || isNumber;
  const r1 = either ? x : undefined;
  const r2: string | number | undefined = r1;
  const r3 = either && x;
  const r4: string | number | false = r3;
}
