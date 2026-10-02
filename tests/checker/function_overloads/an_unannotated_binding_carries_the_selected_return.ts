declare function pick(o: { a: number; init: string }): 'first';
declare function pick(o: { a: number }): 'second';
const picked = pick({ a: 1 });
export const check: 'second' = picked;
