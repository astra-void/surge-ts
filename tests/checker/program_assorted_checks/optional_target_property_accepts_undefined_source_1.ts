// Without `exactOptionalPropertyTypes` an optional target property accepts an
// explicit `undefined`, so a required `T | undefined` source property satisfies
// it. The reverse (a genuinely wrong type) must still report.
type Row = { sheetName?: string; n: number };
declare const src: { sheetName: string | undefined; n: number };
export const ok: Row = src;
