// isSimpleTypeRelatedTo rejects a `never` target before the assignable-only
// `any`-source rule, so `any` is not assignable to `never`: `[any] extends
// [never]` is false, and expect-type's `IsAny<any>` is true.
type IsNever<T> = [T] extends [never] ? true : false;
declare const secret: unique symbol;
type IsAny<T> = [T] extends [typeof secret] ? (IsNever<T> extends true ? false : true) : false;

export const anyIsNotNever: IsNever<any> = false;
export const neverIsNever: IsNever<never> = true;
export const anyIsAny: IsAny<any> = true;
export const numberIsNotAny: IsAny<number> = false;
export const wrong: IsNever<any> = true;

declare const a: any;
export const assigned: never = a;

declare function takesNever(value: never): void;
declare const anyList: any[];
declare const text: string;
declare const neverValue: never;
takesNever(neverValue);
export const list: never[] = anyList;
export const emptyList: never[] = [];
export const pair: [never] = [a];
export const holder: { value: never } = { value: a };
export function returnsNever(): never {
  return a;
}
export const thunk: () => never = () => a;
export const fromNull: never = null;
export const fromText: never = text;
export const fine: string = a;
export const alsoFine: never = neverValue;

type IsNeverDistributive<T> = T extends never ? true : false;
export const nakedAny: IsNeverDistributive<any> = true;
export const nakedAnyFalse: IsNeverDistributive<any> = false;
type ElementIsNever<T> = T extends [infer E] ? ([E] extends [never] ? "never" : "value") : "none";
export const inferredAny: ElementIsNever<[any]> = "value";
