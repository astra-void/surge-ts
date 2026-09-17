declare const key: symbol;
declare const label: string;
declare const count: number;
declare const either: symbol | string;

const joinedLeft = key + "";
const joinedRight = "" + key;
const joinedUnion = either + "x";
const lessThan = key < count;
const atLeast = count >= key;

const noStringOperand = key + count;
const bothSymbols = key + key;
