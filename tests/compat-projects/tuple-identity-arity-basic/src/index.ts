declare var single: [number];
var single: [number];
var single: [number, number];

var frozen: readonly [number];
var frozen: [number];

var frozenArray: readonly number[];
var frozenArray: number[];

var libFrozen: ReadonlyArray<number>;
var libFrozen: readonly number[];

var open: [number, ...(string | boolean)[]];
var open: [number, ...(boolean | string)[]];

var openArity: [number, ...string[]];
var openArity: [number, string, ...string[]];

var openFrozen: readonly [number, ...string[]];
var openFrozen: [number, ...string[]];

export {};
