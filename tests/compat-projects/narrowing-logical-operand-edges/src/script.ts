declare var strOrNumOrBool: string | number | boolean;
declare var strOrNumOrBoolOrC: string | number | boolean | { c: 1 };
declare var numOrBool: number | boolean;
declare var strOrNum: string | number;

if (typeof strOrNumOrBool !== "string" && typeof strOrNumOrBool !== "number") {
  const b: boolean = strOrNumOrBool;
} else {
  strOrNum = strOrNumOrBool;
}

if (
  typeof strOrNumOrBoolOrC !== "string" &&
  typeof strOrNumOrBoolOrC !== "number" &&
  typeof strOrNumOrBoolOrC !== "boolean"
) {
  const c: { c: 1 } = strOrNumOrBoolOrC;
} else {
  const s: string | number | boolean = strOrNumOrBoolOrC;
}

if (typeof strOrNumOrBool === "string" || typeof strOrNumOrBool === "number") {
  strOrNum = strOrNumOrBool;
} else {
  const b: boolean = strOrNumOrBool;
}

if (typeof strOrNumOrBool !== "string" && numOrBool !== strOrNumOrBool) {
  numOrBool = strOrNumOrBool;
}
