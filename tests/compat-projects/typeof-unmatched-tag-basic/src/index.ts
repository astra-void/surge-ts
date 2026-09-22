class C {
  x = 1;
}

export function splits(
  strOrC: string | C,
  numOrBoolOrC: number | boolean | C,
  strOrNum: string | number,
): void {
  if (typeof strOrC === "Object") {
    const a: void = strOrC;
  } else {
    const b: void = strOrC;
  }
  if (typeof numOrBoolOrC === "Nope") {
    const c: void = numOrBoolOrC;
  } else {
    const d: void = numOrBoolOrC;
  }
  if (typeof strOrNum === "Nope") {
    const e: void = strOrNum;
  } else {
    const g: void = strOrNum;
  }
}
