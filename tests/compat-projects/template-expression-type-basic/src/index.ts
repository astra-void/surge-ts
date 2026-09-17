declare const sym: symbol;
declare const maybeSym: symbol | string;
declare const n: number;

export const t1 = `${sym}`;
export const t2 = `a${maybeSym}b`;
export const t3: "ab" = `a${"b"}`;
export const t4: "a1" = `a${1}`;
export const t5: number = `x`;
export const t6: `a${number}` = `a${n}`;
export const t7: "done" = `do${"ne"}`;
export const t8: string = `${n}${String(sym)}`;
