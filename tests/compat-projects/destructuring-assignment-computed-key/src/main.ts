declare const source: { a: number; b: string };
declare const either: "a" | "b";
const aKey = "a";

let n: number;
let text: string;
({ [aKey]: n } = source);
({ [either]: text } = source);

export const read: number = n;
export const readText = text;
