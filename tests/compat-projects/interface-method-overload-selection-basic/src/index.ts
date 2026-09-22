interface Parser {
  parse(input: number[]): string;
  parse(input: string[]): boolean;
}

interface Picker {
  pick<T>(items: T[]): T;
  pick(items: unknown[]): unknown;
}

interface Builder {
  items<const Items extends readonly unknown[]>(items: Items): Items;
}

declare const parser: Parser;
declare const picker: Picker;
declare const builder: Builder;

export const parsed: number = parser.parse(['x']);
export const picked: string = picker.pick([1]);
export const built: readonly [1, 'a'] = builder.items([1, 'a']);

declare const form: HTMLFormElement;
const values = Object.fromEntries(new FormData(form));
export const title = values.title;

interface HeadersLike {
  [Symbol.iterator](): IterableIterator<[string, string]>;
}

export function toRecord(headers: HeadersLike | Record<string, string | undefined>) {
  if (Symbol.iterator in headers) {
    return Object.fromEntries(headers);
  }
  return headers;
}

type Loader<T> = () => T | Promise<T>;
export const load: Loader<'data'> = () => Promise.resolve('data');
