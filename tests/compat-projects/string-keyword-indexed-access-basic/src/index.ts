export type Flat = Record<string, number | boolean>;
export type FlatValue = Flat[string];
export const fromRecord: FlatValue = 1;
export const alsoABoolean: FlatValue = true;

export interface Indexed {
  [key: string]: string;
}
export type IndexedValue = Indexed[string];
export const fromInterface: IndexedValue = 'x';

export type Nested = Record<string, Record<string, number>>;
export const fromNested: Nested[string][string] = 2;

export type Mapped = { [K in 'a' | 'b']: number };
export const aLiteralKeyStillReads: Mapped['a'] = 3;

export type Counts = Record<string, number>;
declare const count: Counts[string];
export const theValueTypeIsTheRecordsOwn = count.nope;
