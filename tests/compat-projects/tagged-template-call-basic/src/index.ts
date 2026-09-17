declare function tag(strings: TemplateStringsArray, n: number): string;
export const t1 = tag`x${"a"}`;
export function g() { return tag`y${true}`; }
export const t3: number = tag`z${1}`;
declare function rest(strings: TemplateStringsArray, ...values: number[]): number;
export const t4 = rest`a${1}b${"two"}c${3}`;
declare const sym: symbol;
export const t6 = rest`a${sym}`;
const html = (s: TemplateStringsArray, ...v: unknown[]) => s.join("");
export const t7: number = html`<p>${1}</p>`;
