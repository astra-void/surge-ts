declare const text: string;
declare const flag: boolean;
declare const thing: object;

export const fromAString = +text;
export const negatedString = -text;
export const fromABoolean = +flag;
export const fromAnObject = +thing;

type Selector = (data: string) => [string, number];
export const selector: Selector = (data) => [data, +data];

export const coercesToNumber: number = +text;
