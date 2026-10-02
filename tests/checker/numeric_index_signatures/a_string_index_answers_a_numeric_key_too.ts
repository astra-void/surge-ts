// @noImplicitAny: true
interface StringKeyed { [key: string]: number }
declare const stringKeyed: StringKeyed;
export const a: number = stringKeyed[3];
export const b: string = stringKeyed[4];
