declare const Ctor: new (size: number) => { fromCtor: number; shared: string };

export class Derived extends Ctor {
  shared = 1;
  extra = true;
}

export const viaBase: string = new Derived(1).fromCtor;
export const own: number = new Derived(1).extra;
export const unknownMember = new Derived(1).nope;
export const wrongArgument = new Derived("large");

export function usesLater(derived: Derived) {
  return derived.fromCtor;
}

declare const Loose: any;

export class Open extends Loose {
  own = 1;
}

export const anything: string = new Open().anything;
export const declared: string = new Open().own;
