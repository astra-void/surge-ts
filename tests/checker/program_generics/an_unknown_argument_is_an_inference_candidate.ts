declare function replaceData<TData>(prevData: TData | undefined, data: TData): TData;
export function memo<TData>(prev: { data: TData } | undefined, placeholder: unknown): TData {
  return replaceData(prev?.data, placeholder) as TData;
}
declare function both<T>(a: T, b: T): T;
export function widen<U>(u: U, x: unknown) {
  both(u, x);
  both(u, 1);
}
