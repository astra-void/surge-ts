// `Record<number, T>` is an index signature, not an empty property set.
declare const rec: Record<number, boolean>;
export const a: boolean = rec[1];
export const b: boolean = rec[0];
