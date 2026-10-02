type Holder = { items: [string, number] };
declare const holder: Holder;
declare function two(a: string, b: number): void;
export function f() { two(...holder.missing); }
