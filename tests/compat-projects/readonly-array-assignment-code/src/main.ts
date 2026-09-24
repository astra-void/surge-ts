declare let ma: string[];
declare let ra: readonly string[];
declare let mt: [string, string];
declare let rt: readonly [string, string];

ma = ra;
ma = rt;
mt = ra;
mt = rt;
ra = ma;

export const declared: string[] = ra;
