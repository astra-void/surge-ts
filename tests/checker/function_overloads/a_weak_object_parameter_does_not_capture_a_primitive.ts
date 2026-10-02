// A wrong pick is worse than no pick, so selection is stricter than the
// fold's assignability where tsc is: a weak object type (all properties
// optional) accepts nothing that shares no property with it. `readFileSync(p,
// 'utf8')` picked the `Buffer` overload without this.
declare function read(p: string, o?: { encoding?: null; flag?: string } | null): 1;
declare function read(p: string, o: { encoding: 'utf8'; flag?: string } | 'utf8'): 2;
export const text: 2 = read('x', 'utf8');
export const buffer: 1 = read('x');
