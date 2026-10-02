// Two literal arguments for one parameter still meet at the base primitive.
declare function pair<T>(a: T, b: T): T;
export const value: number = pair(1, 2);
