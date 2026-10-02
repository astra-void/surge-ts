// @noImplicitAny: true
interface BothKeys { [index: number]: string; [key: string]: string | number }
declare const bothKeys: BothKeys;
export const a: number = bothKeys[2];
