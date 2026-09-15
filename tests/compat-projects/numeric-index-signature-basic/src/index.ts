// A numeric index signature answers a numeric key and nothing else: a string
// key it cannot answer is an implicit `any`, and a `for…in` over a type whose
// only index signature is numeric binds its key as `number`.

interface NumberKeyed {
  [index: number]: string;
}

declare const numberKeyed: NumberKeyed;
declare const numericKey: number;
declare const stringKey: string;

export const byLiteral: string = numberKeyed[0];
export const byNumber: string = numberKeyed[numericKey];
export const byLiteralWrong: number = numberKeyed[1];
numberKeyed[stringKey] = "value";

export function iterate() {
  for (const key in numberKeyed) {
    const value: string = numberKeyed[key];
    const asNumber: number = key;
    const asString: string = key;
    return value + asNumber + asString;
  }
  return "";
}

interface BothKeys {
  [index: number]: string;
  [key: string]: string | number;
}

declare const bothKeys: BothKeys;
export const numericWins: string = bothKeys[0];
export const stringFallback: string | number = bothKeys[stringKey];
export const numericWrong: number = bothKeys[2];

interface StringKeyed {
  [key: string]: number;
}

declare const stringKeyed: StringKeyed;
export const numericThroughString: number = stringKeyed[3];
export const stringThroughString: number = stringKeyed[stringKey];
export const stringKeyedWrong: string = stringKeyed[4];

export function iterateStringKeyed() {
  for (const key in stringKeyed) {
    const asString: string = key;
    return asString;
  }
  return "";
}
