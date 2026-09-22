declare let opaque: unknown;
declare let holder: { value: unknown };

export const sum = opaque + 1;
export const negated = -opaque;
export const product = holder.value * 2;
export const parenthesized = (opaque) * 2;
export const called = opaque();
export const constructed = new opaque();
export const element = opaque[0];
export const chained = opaque?.name;
export const joined = opaque + "suffix";
