// @noImplicitAny: true
interface NumberKeyed { [index: number]: string }
declare const numberKeyed: NumberKeyed;
declare const numericKey: number;
declare const stringKey: string;
export const a: string = numberKeyed[0];
export const b: string = numberKeyed[numericKey];
