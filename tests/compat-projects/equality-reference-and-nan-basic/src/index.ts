declare const target: object;
declare const list: number[];
declare const value: number;

const againstObject = target === {};
const againstArray = list == [];
const againstFunction = target !== function () {};
const objectFirst = {} === target;
const againstArrow = target === (() => 1);

const nanRight = value === NaN;
const nanLeft = NaN !== value;
const nanLoose = value == NaN;
const nanShadowed = (NaN: number) => value === NaN;
