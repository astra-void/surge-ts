// `+` concatenates a string with a `bigint` (and with a `number | bigint`, the
// zod issue-`minimum` shape) exactly as tsc does, while the arithmetic
// `number + bigint` stays an error — bigint is deliberately concatenation-only.
declare const mixed: number | bigint;
declare const n: number;
export const a = n + mixed;
