type Item = { a: number };
declare const it: Item;
declare function list<T extends Item[] | []>(v: T): T;
export const a = list([it, it]);
