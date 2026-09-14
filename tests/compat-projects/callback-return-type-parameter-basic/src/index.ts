interface Reader<TValue> {
  read<TResult>(map: (value: TValue) => TResult): TResult;
}

declare const reader: Reader<string>;
export const readWidth: number = reader.read((value) => value.length);
export const readRejected: string = reader.read((value) => value.length);

declare function pick<TValue, TResult>(
  value: TValue,
  map: (value: TValue) => TResult,
): TResult;
export const picked: number = pick('abc', (value) => value.length);
export const pickedRejected: string = pick('abc', (value) => value.length);

class Box<TValue> {
  constructor(readonly value: TValue) {}
  map<TResult>(map: (value: TValue) => TResult): TResult {
    return map(this.value);
  }
}
export const mapped: number = new Box('abc').map((value) => value.length);
export const mappedRejected: string = new Box('abc').map((value) => value.length);

declare const annotated: Reader<string>;
export const annotatedWidth: number = annotated.read((value: string) => value.length);

interface Loader<TData> {
  load: (key: string) => Promise<TData>;
}
declare function observe<TData>(loader: Loader<TData>): (listener: (value: TData) => void) => void;

export const subscribe = observe({ load: async (key: string) => key.length });
subscribe((value) => {
  const asNumber: number = value;
  void asNumber;
});
subscribe((value) => {
  const asString: string = value;
  void asString;
});
