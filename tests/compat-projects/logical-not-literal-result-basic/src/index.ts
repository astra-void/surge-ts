declare const shape: { size: number };
declare const optionalShape: { size: number } | undefined;
declare const text: string;
declare const callback: () => void;

// `!` of an always-truthy operand is `false`, of an always-falsy one `true`.
const notOne: false = !1;
const notZero: true = !0;
const notShape: false = !shape;
const notCallback: false = !callback;
const notCallbackWrong: true = !callback;

// An operand that can be either way gives `boolean`.
const notOptional: true = !optionalShape;
const notText: true = !text;

let reassigned = !shape;
reassigned = true;
const collected: boolean[] = [!shape, !optionalShape];
