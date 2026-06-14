type A = (value: string) => string;
type B = (value: string) => string;

declare const fn: A | B;

fn(123);
