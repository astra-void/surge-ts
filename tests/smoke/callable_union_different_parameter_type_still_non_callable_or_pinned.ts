type StringParam = (value: string) => string;
type NumberParam = (value: number) => string;

declare const fn: StringParam | NumberParam;

fn("x");
