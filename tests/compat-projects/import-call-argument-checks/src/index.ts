declare const specifier: number;
declare const parts: string[];

export const fromNumber = import(specifier);
export const spread = import(...parts);
export const typed = import<string>("./dep.js");
export const withOptions = import("./dep.js", { with: { type: 1 } });
