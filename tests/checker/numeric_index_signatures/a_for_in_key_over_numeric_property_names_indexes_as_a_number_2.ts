// tsc's `isForInVariableForNumericPropertyNames`: the binding stays a
// `string`, and the access through it reads the numeric index.
// The key is a `string`, not a `number`.
// @noImplicitAny: true
interface NumberKeyed { [index: number]: string }
declare const numberKeyed: NumberKeyed;
declare const numericKey: number;
declare const stringKey: string;
export function f() {
  for (const key in numberKeyed) {
    const asNumber: number = key;
    return asNumber;
  }
  return 0;
}
