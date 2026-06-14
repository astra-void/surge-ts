type StringFn = (value: string) => string;

declare const fn: StringFn | Missing;

fn("x");
