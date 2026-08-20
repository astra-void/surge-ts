export namespace util {
  type AssertEqual<A, B> = (<V>() => V extends A ? 1 : 2) extends <V>() => V extends B ? 1 : 2
    ? true
    : false;

  export const assertEqual = <A, B>(_: AssertEqual<A, B>): void => {};
  export function identity<T>(value: T): T {
    return value;
  }
}

util.assertEqual<{ a: string }, { a: string }>(true);

export const identified: string = util.identity("value");
