type QueryKey = ReadonlyArray<unknown>;
type WithRequired<TTarget, TKey extends keyof TTarget> = TTarget & {
  [_ in TKey]: {};
};

interface QueryOptions<TData = unknown, TQueryKey extends QueryKey = QueryKey> {
  queryKey?: TQueryKey;
  data?: TData;
}

interface FetchQueryOptions<
  TData = unknown,
  TQueryKey extends QueryKey = QueryKey,
> extends WithRequired<QueryOptions<TData, TQueryKey>, 'queryKey'> {
  initialPageParam?: never;
}

interface EnsureQueryDataOptions<
  TData = unknown,
  TQueryKey extends QueryKey = QueryKey,
> extends FetchQueryOptions<TData, TQueryKey> {
  revalidateIfStale?: boolean;
}

export class QueryClient {
  ensure<TData, TQueryKey extends QueryKey = QueryKey>(
    options: EnsureQueryDataOptions<TData, TQueryKey>,
  ) {
    return this.fetch(options);
  }

  fetch<TData, TQueryKey extends QueryKey = QueryKey>(
    options: FetchQueryOptions<TData, TQueryKey>,
  ) {
    return options;
  }

  ensureConcrete<TData>(
    options: EnsureQueryDataOptions<TData, QueryKey>,
  ) {
    return this.fetch(options);
  }
}

declare const holder: {
  fetch<TQueryKey extends QueryKey = QueryKey>(
    options: FetchQueryOptions<unknown, TQueryKey>,
  ): void;
};
export function throughObject<TQueryKey extends QueryKey>(
  options: EnsureQueryDataOptions<unknown, TQueryKey>,
) {
  return holder.fetch(options);
}

declare const defaulted: { identity<T = string>(value: T): T };
export const identityNumber = defaulted.identity(42);
export const identityString = defaulted.identity('s');

declare const returnOnly: { make<T = string>(): T[] };
export const madeStrings: string[] = returnOnly.make();

declare const openTarget: FetchQueryOptions<unknown, QueryKey>;
export const readKey: {} = openTarget.queryKey;

declare const mismatched: { queryKey: unknown[] };
export const rejected: FetchQueryOptions<string, string[]> = mismatched;
