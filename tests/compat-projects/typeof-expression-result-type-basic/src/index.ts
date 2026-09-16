type Kind = "string" | "number" | "bigint" | "boolean" | "symbol" | "undefined" | "object" | "function";

declare const value: unknown;
declare const text: string | number;

// `typeof` evaluates to the union of its eight results, not to `string`.
const kind: Kind = typeof value;
function kindOf(input: unknown): Kind {
  return typeof input;
}
const tooNarrow: "string" = typeof value;
const widened: number = typeof value;

if (typeof text === "string") {
  const narrowed: string = text;
}
const tag = `${typeof value}`;
const record: Record<string, number> = {};
record[typeof value] = 1;
