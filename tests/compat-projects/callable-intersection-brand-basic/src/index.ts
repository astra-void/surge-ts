export {}
type FnAlias<P> = (props: P) => null;
type ClassAlias<P> = { new (props: P): { render(): null } };
type Component<P> = ClassAlias<P> | FnAlias<P>;

type WithInitialProps<P> = Component<P> & { getInitialProps?(): P };
const app: WithInitialProps<{ session: string }> = () => null;
app.getInitialProps = () => ({ session: 'x' });

type CallableBrand = FnAlias<{ a: number }> & { extra?: number };
declare const callable: CallableBrand;
const readExtra: number | undefined = callable.extra;

type QueryKey = ReadonlyArray<unknown>;
type WithRequired<TTarget, TKey extends keyof TTarget> = TTarget & {
  [_ in TKey]: {};
};
interface QueryOptions<TData = unknown, TQueryKey extends QueryKey = QueryKey> {
  queryKey?: TQueryKey;
  data?: TData;
}
interface FetchQueryOptions<TData = unknown, TQueryKey extends QueryKey = QueryKey>
  extends WithRequired<QueryOptions<TData, TQueryKey>, 'queryKey'> {
  initialPageParam?: never;
}
declare const mismatched: { queryKey: unknown[] };
export const rejected: FetchQueryOptions<string, string[]> = mismatched;

type StringBrand = string & { _?: never };
declare const branded: StringBrand;
export const asString: string = branded;
