type InitialDataFunction<T> = () => T | undefined;
interface Options<TData> { initialData?: TData | InitialDataFunction<TData> }
interface Cached<T> { data: T | undefined }
declare function build<TData>(options: Options<TData>): Cached<TData>;
export function ensure<TData>(options: Options<TData>): Promise<TData> | undefined {
  const cached = build(options).data;
  return cached === undefined ? undefined : Promise.resolve(cached);
}
declare function pick<T>(x: T | InitialDataFunction<T>): T;
export function read<D>(x: D | InitialDataFunction<D>): D {
  return pick(x);
}
export function readWritten<D>(x: D | (() => D | undefined)): D {
  return pick(x);
}
export const fromLiteral: string = pick(() => 1);
