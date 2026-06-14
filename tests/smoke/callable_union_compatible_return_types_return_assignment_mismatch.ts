type StringFn = () => string;
type NumberFn = () => number;

declare const fn: StringFn | NumberFn;

const out: string = fn();
