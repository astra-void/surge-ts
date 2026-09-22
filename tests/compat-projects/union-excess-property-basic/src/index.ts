type A = { a: number };
type B = { b: string };
type One = { kind: "one"; x: number };
type Two = { kind: "two"; y: string };

declare function takes(value: A | B): void;

export const oneMember: A | B = { a: 1, zz: 1 };
export const bothMembers: A | B = { a: 1, b: "s" };
export const bothPlusExcess: A | B = { a: 1, b: "s", zz: 1 };
export const withUndefined: A | undefined = { a: 1, zz: 1 };
export const withPrimitive: A | string = { a: 1, zz: 1 };

export const otherMembersProperty: One | Two = { kind: "one", x: 1, y: "s" };
export const discriminatedExcess: One | Two = { kind: "one", x: 1, zz: 1 };
export const discriminatedOk: One | Two = { kind: "two", y: "s" };

takes({ a: 1, zz: 1 });

export const indexMember: A | { [key: string]: unknown } = { a: 1, zz: 1 };
export const anyMember: A | any = { a: 1, zz: 1 };
export const nested: { inner: A | B } = { inner: { a: 1, zz: 1 } };
export const inArray: (A | B)[] = [{ a: 1, zz: 1 }];
