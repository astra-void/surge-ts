interface First {
  (): First;
  prototype: First;
  onlyFirst: string;
}
interface Second {
  (): Second;
  prototype: Second;
  onlySecond: number;
}
interface Derived extends First {
  prototype: Derived;
  onlyDerived: number;
}
interface Built {
  built: boolean;
}
declare const first: First;
declare const second: Second;
declare const derived: Derived;
declare const builder: { new (): Built };

export function byPrototype(value: First | Second, other: Second | Derived) {
  const readFirst: number = value instanceof first && value.onlyFirst;
  const readSecond: string = value instanceof second && value.onlySecond;
  const readDerived: string = value instanceof derived && value.onlyDerived;
  const readOther: string = other instanceof derived && other.onlyDerived;
  if (value instanceof first) {
    const narrowed: number = value;
    return [readFirst, readSecond, readDerived, readOther, narrowed];
  }
  const rest: number = value;
  return [rest];
}

export function byConstructSignature(value: Built | string) {
  if (value instanceof builder) {
    const narrowed: number = value;
    return narrowed;
  }
  const rest: number = value;
  return rest;
}
