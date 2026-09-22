interface Untyped {
  key;
  optionalKey?;
}
declare const untyped: Untyped;

export const read: number = untyped.key;
export const readOptional: string = untyped.optionalKey;
export const wholeValue: string = untyped;
export const fromNumber: Untyped = 3;
export const missingKey: Untyped = { optionalKey: 1 };
export const present: Untyped = { key: "anything" };

export const literal: { first; second: number } = 3;
export const literalMissing: { first; second: number } = { second: 1 };
export const mixed: { label; (): string } = 3;
