declare function make<T>(value: T): T[];
declare const factories: {
  make<T>(value: T): T[];
};

export const makeStrings = make<string>;
export const makeStringsFromMember = factories.make<string>;

export const strings = makeStrings('a');
export const alsoStrings = makeStringsFromMember('b');

export const wrong: number[] = make<string>('a');

export function callsWithoutArguments(): string[] {
  return make<string>();
}
