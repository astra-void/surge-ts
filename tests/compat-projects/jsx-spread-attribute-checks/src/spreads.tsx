declare const always: { x: string; y: number };
declare const sometimes: { x?: string };
declare const numbered: { x: number };
declare const nothing: null;

// A spread that always writes a property overwrites the attribute written
// before it: TS2783 at that attribute.
export const overwritten = <box x="a" {...always} />;
export const kept = <box x="a" {...sometimes} />;
export const after = <box {...always} x="a" />;

// The attributes object carries what the spread wrote, and that is what is
// related: no mismatch for the overwritten `1`, one for the overwritten `"a"`.
export const rescued = <box x={1} {...always} />;
export const broken = <box x="a" {...numbered} />;

// Only an object spreads: TS2698 at the expression.
export const fromNull = <box x="a" {...nothing} />;
export const fromString = <box x="a" {..."text"} />;
