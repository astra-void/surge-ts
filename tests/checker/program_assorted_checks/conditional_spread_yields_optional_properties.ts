// A conditional spread contributes optional properties, so `in`-guarded access
// on the spread member resolves (tsc infers `list?: string[]`).
declare const cond: boolean;
const step = { title: "t", ...(cond ? { list: ["a"] } : {}) };
const items: string[] | undefined = step.list;
const title: string = step.title;
