declare const value: object;

export const assigned: string = value;
export const read = value.nope;

declare function takesObject(input: object): void;
takesObject({ a: 1 });
takesObject([1, 2]);
takesObject(() => 1);

class Holder<T extends object> {
  item(): T {
    return null as any;
  }
}
declare const holder: Holder<object>;
export const throughConstraint = holder.item().nope;

declare const literal: {};
export const literalAssigned: string = literal;
export const literalRead = literal.nope;
