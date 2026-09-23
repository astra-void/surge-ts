declare const text: string;
declare const count: number;

export const bothNull = null + null;
export const nullAndUndefined = null + undefined;
export const bothUndefined = undefined + undefined;
export const nullAndNumber = null + count;
export const numberAndUndefined = count + undefined;
export const stringAndNull = text + null;
export const nullAndString = null + text;
export const subtractNull = null - 1;
export const multiplyUndefined = undefined * 2;
