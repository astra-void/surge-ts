type ClassValue = string | number | null | boolean | undefined | ClassValue[];

export function cn(...inputs: ClassValue[]): string {
  return inputs.filter(Boolean).join(" ");
}

const a = cn("a", "b", null, undefined);

export function tag(first: string, ...rest: number[]): string {
  return first + rest.length;
}
const b = tag("x", 1, 2, 3);
const c = tag("x");

const join = (sep: string, ...parts: string[]) => parts.join(sep);
const d = join("-", "p", "q");

export { a, b, c, d };
