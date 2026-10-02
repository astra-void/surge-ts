// @noImplicitAny: true
interface BothKeys { [index: number]: string; [key: string]: string | number }
declare const bothKeys: BothKeys;
declare const stringKey: string;
export const a: string = bothKeys[0];
export const b: string | number = bothKeys[stringKey];
