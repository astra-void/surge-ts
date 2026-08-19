interface Formatted<T, U = string> {
  _errors: U[];
  _value: T;
}

interface Err<T = unknown> {
  format(): Formatted<T>;
  format<U>(mapper: (issue: string) => U): Formatted<T, U>;
}

declare const err: Err<{ name: string }>;

export const mapped: Formatted<{ name: string }, number> = err.format(() => 5);
