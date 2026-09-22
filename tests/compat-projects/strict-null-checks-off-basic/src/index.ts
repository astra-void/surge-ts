declare let maybe: number | undefined;
declare let record: { a?: number; b: string | null };
declare function lookup(key?: string): number | undefined;
declare let opaque: unknown;

export const fromUnion: number = maybe;
export const fromOptional: number = record.a;
export const fromNull: string = record.b;
export const method = maybe.toFixed();
export const wrong: string = lookup();
export const nothing: string = undefined;

let widened = undefined;
widened = 1;
widened = "s";

let unassigned: string;
export const read = unassigned.length;

export function optional(x?: number) {
  return x.toFixed();
}

export const property = opaque.name;
export const called = opaque();
export const constructed = new opaque();
for (const item of opaque) {
}
export const element = opaque[0];

const empty: string = [];
const list = [];
list.push(1);
