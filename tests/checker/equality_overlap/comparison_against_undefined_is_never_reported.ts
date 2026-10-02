// tsc's equality rule accepts a nullable operand outright, so comparing any
// value to `undefined` is never TS2367.
interface SchemaObject { type?: string }
declare const schema: SchemaObject;
declare const count: number;
declare const marker: symbol;
export const a = schema !== undefined;
export const b = count === undefined;
export const c = marker === undefined;
export const d = undefined === count;
