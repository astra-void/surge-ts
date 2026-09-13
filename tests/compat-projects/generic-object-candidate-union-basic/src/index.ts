declare function shallowEqual<T extends Record<string, unknown>>(a: T, b: T | undefined): boolean;

export const sameShape = shallowEqual({ a: 1 }, { a: 2 });
export const widerShape = shallowEqual({ a: 1 }, { a: 1, b: 2 });
export const missing = shallowEqual({ a: 1 }, undefined);

declare function subject<T>(initial: T): { next: (value: T) => void; get: () => T };

export function fresh(): number {
  const value = subject({ n: 1 });
  value.next({ n: 2 });
  return value.get().n;
}

export function asserted(): 1 {
  const kept = subject({ n: 1 } as const);
  return kept.get().n;
}
