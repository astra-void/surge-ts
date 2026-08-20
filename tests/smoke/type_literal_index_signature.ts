type InexactPartial<T> = { [P in keyof T]?: T[P] | undefined };
type LoosePartial<T extends object> = InexactPartial<T> & { [k: string]: unknown };

interface Bag {
  bag: LoosePartial<{ minimum: number; maximum: number }>;
}

export function read(b: Bag): unknown {
  const { minimum, multipleOf } = b.bag;
  void minimum;
  return multipleOf;
}
