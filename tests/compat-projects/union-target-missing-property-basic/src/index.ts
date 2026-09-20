interface Plain {
  a: number;
  c: boolean;
}
type One = { kind: "one"; x: number };
type Two = { kind: "two"; y: string };
type Left = { left: number; shared: string };
type Right = { right: string; shared: string };

declare function takes(value: One | Two): void;

export const discriminated: One | Two = { kind: "two" };
export const byProperty: Left | Right = { left: 1 };
export const sharedOnly: Left | Right = { shared: "s" };
export const withUndefined: Left | undefined = { shared: "s" };
export const single: One = { kind: "one" };

takes({ kind: "one" });

export function returns(): One | Two {
  return { kind: "two" };
}

export const nestedUnion: { inner: One | Two } = { inner: { kind: "two" } };
export const nestedPlain: { inner: Plain } = { inner: { a: 1 } };
export const nestedDeep: { deep: { inner: Plain } } = { deep: { inner: { a: 1 } } };
export const nestedQuoted: { "quoted-key": Plain } = { "quoted-key": { a: 1 } };
export const inArray: { list: Plain[] } = { list: [{ a: 1 }] };
export const unionArray: (One | Two)[] = [{ kind: "one" }];
