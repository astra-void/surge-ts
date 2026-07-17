type QueryConfig<T> = {
  initial: T;
  refetchOnMount?: boolean;
};

type QueriesResultMap<T extends Record<string, QueryConfig<unknown>>> = {
  [K in keyof T]: { data: T[K]["initial"]; stale: boolean };
};

type ConfigValue<C> = C extends QueryConfig<infer V> ? V : never;
const extracted: ConfigValue<QueryConfig<number>> = 1;

declare function useQueries<T extends Record<string, QueryConfig<unknown>>>(
  configs: T,
): QueriesResultMap<T>;

const results = useQueries({
  count: { initial: 0 },
  name: { initial: "x", refetchOnMount: true },
});

const count: number = results.count.data;
const name: string = results.name.data;
const stale: boolean = results.name.stale;

type Wrapped<T> = { value: T };
type DoubleWrapped<T> = Wrapped<Wrapped<T>>;
declare function wrapTwice<T>(seed: T): DoubleWrapped<T>;
const doubled = wrapTwice(5);
const inner: number = doubled.value.value;

const bad: string = results.count.data;

export { count, name, stale, inner, extracted, bad };
