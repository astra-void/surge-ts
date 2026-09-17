enum Direction { Up, Down }
type MinusOne = -1;

const negativeOk: -1 = -1;
const negativeIntoPositive: 1 = -1;
const positiveIntoNegative: -1 = 1;
const fraction: -1.5 = -1.5;
const negativeZero: 0 = -0;
const unaryPlus: 1 = +1;
const unaryPlusMismatch: -1 = +1;
const hex: 16 = -0x10;
const outsideUnion: -1 | 2 = -3;
const aliased: MinusOne = -2;
const typePositionMismatch: -5 = 5;
const enumOutOfRange: Direction = -1;

function takesSign(sign: -1 | 1): void {}
takesSign(-1);
takesSign(-2);

const readonlyTuple = [-1, 2] as const;
const tupleMismatch: readonly [1, 2] = readonlyTuple;

// A signed literal is as fresh as an unsigned one: it widens where `1` widens.
const inferredConst = -1;
const constKeepsLiteral: 1 = inferredConst;
let widened = -1;
widened = 5;
const returnsWidened = () => -1;
const widenedReturn: () => number = returnsWidened;

const pair: [number, string] = [1, "a"];
pair[-1] = 2;

// A signed computed key names the property `-1`.
const computedKey: { [-1]: string } = { [-1]: "x" };
const quotedTarget: { "-1": string } = { [-1]: "x" };

// A mapped type over number keys binds the key parameter to the number literal.
type KeyOf<TValue, TType extends Record<PropertyKey, PropertyKey>> = {
  [K in keyof TType]: TValue extends TType[K] ? K : never;
}[keyof TType];
type Inverted<TType extends Record<PropertyKey, PropertyKey>> = {
  [TValue in TType[keyof TType]]: KeyOf<TValue, TType>;
};
const codes = { PARSE: -32700, OK: 200 } as const;
const inverted: Inverted<typeof codes> = { [-32700]: "PARSE", [200]: "OK" };
type Echo = { [K in 1 | 2]: K };
const echoMismatch: Echo = { 1: 2, 2: 2 };
