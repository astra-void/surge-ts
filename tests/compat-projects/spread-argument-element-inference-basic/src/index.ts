declare const numbers: number[];
declare const names: Set<string>;
declare function rest<T>(...items: T[]): T[];

export const fromArraySpread: string = rest(...numbers);
export const fromArguments: string = rest(1, 2);
export const fromSpreadAndArgument: string = rest(...numbers, 5);
export const fromSetSpread: string = rest(...names);

export const copied: string = [...numbers];
export const extended: string = [...numbers, 1];
export const fromSetLiteral: string = [...names];
export const fromStringLiteral: string = [..."abc"];

declare const tuple: [string, number];
export const fromTupleLiteral: string = [...tuple];
