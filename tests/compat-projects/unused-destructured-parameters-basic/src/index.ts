type O = { a: number; b: number; c: number };
type N = { a: number; n: { p: number; q: number } };

export const oneUnusedElement = ({ a }: O) => 1;
export const allUnused = ({ a, b }: O) => 1;
export const someUnused = ({ a, b }: O) => a;
export const renamedUnderscore = ({ a: _a, b }: O) => b;
export const shorthandUnderscore = ({ _x }: { _x: number }) => 1;
export const objectRest = ({ a, ...rest }: O) => a;
export const objectRestUnderscore = ({ a, ..._rest }: O) => a;
export const objectRestUsed = ({ a, ...rest }: O) => rest;
export const arrayAllUnused = ([x, y]: number[]) => 1;
export const arrayUnderscore = ([_x, y]: number[]) => y;
export const arrayRestUnderscore = ([x, ..._rest]: number[]) => x;
export const arrayRest = ([x, ...rest]: number[]) => x;
export const nestedAllUnused = ({ a, n: { p, q } }: N) => a;
export const nestedOneUnused = ({ a, n: { p, q } }: N) => a + p;
