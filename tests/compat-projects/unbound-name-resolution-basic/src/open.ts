// `Omit` and `Partial` keep an open source open.
interface Open {
  [key: string]: unknown;
  known: string;
}
declare const omitted: Omit<Open, 'known'>;
declare const partial: Partial<Open>;
export const a = omitted['anything'];
export const b = partial['anything'];
