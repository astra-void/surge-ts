declare let toStrings: (x: number) => string[];
declare let pair: (x: number, y: string) => number;
declare let fromStrings: (x: string[]) => number;
declare let wrap: <T>(x: T) => T[];
declare let same: <T>(x: T, y: T) => T;
declare let first: <T>(x: T[]) => T;
declare let identity: <T>(x: T) => T;
declare let numeric: (x: number) => number;

export function assignments(): void {
  toStrings = wrap;
  pair = same;
  fromStrings = first;
  numeric = identity;
}

declare function withFew<a, r>(values: a[], haveFew: (values: a[]) => r, haveNone: (reason: string) => r): r;
declare function id<a>(value: a): a;
declare function fail(message: string): never;
declare function apply<T, R>(x: T, fn: (x: T) => R): R;
declare function box<U>(u: U): U[];

export function inference(): void {
  const a: string = withFew([1, 2, 3], id, fail);
  const b: number[] = withFew([1, 2, 3], id, fail);
  const c: string = apply(1, box);
  const d: number[] = apply(1, box);
}

export function declared(): void {
  let x: undefined;
  x = 1;
  x = "";
}
