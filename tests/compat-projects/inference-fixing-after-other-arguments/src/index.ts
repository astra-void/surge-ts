interface CounterState { count: number; inc: () => void }
type Store<T> = {
  subscribe: {
    (listener: (selectedState: T) => void): () => void
    <U>(selector: (state: T) => U, listener: (selectedState: U) => void): () => void
  }
}
declare const store: Store<CounterState>;
declare const counter: CounterState;
declare function watch<T, U>(state: T, selector: (state: T) => U, listener: (selected: U) => void): void;
declare function mapObject<T, U>(obj: { [x: string]: T }, f: (x: T) => U): { [x: string]: U };
declare const rec: { [x: string]: string };
declare function both<T>(a: (x: T) => T, b: (x: T) => T): T;
declare function log(value: number): void;

// A type parameter an earlier argument's parameter names gets its candidate
// there before a later callback fixes it: `U` from the selector's return,
// `T` from `rec` or `counter`.
store.subscribe((state) => state.count, (count) => log(count * 2));
mapObject(rec, (s) => s.length);
watch(counter, (s) => s.count, (c) => log(c));
watch(counter, (s) => s.count, (c) => c.toUpperCase());

// A callback that comes later has not been inferred from when an earlier one
// fixes the parameter, so `T` is fixed at `unknown` by the first callback.
both((x) => 1, (x) => "");
