let { x }: { x?: string | number } = {};
const { a2 }: any = {};
const { b }: { b: number } = { b: "text" };
declare const loose: any;
const { data }: { data: string[] } = loose;
export const count: number = data.length;
const [first, second]: [number, string] = [1, "two"];
const { missing }: { present: number } = { present: 1 };
export const all = [x, a2, b, first, second, missing];
