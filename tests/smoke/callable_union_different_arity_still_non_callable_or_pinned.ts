type NoArg = () => string;
type OneArg = (value: string) => string;

declare const fn: NoArg | OneArg;

fn("x");
