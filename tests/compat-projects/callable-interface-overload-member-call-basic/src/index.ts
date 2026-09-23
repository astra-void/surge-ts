interface Out1<T> {
  kind1: T;
}
interface Out2<T> {
  kind2: T;
}
interface In1<T> {
  initialData: T;
}
interface QueryOptions<TDef extends { input: unknown; output: unknown }> {
  <TQ extends TDef['output'], TData = TQ>(input: TDef['input'], opts: In1<TQ>): Out1<TData>;
  <TQ extends TDef['output'], TData = TQ>(
    input: TDef['input'],
    opts?: { select?: (data: TQ) => TData },
  ): Out2<TData>;
}
declare const holder: { list: { queryOptions: QueryOptions<{ input: void; output: string[] }> } };

const inferred = holder.list.queryOptions();
const fromMember: Out2<string[]> = inferred;
export function inBody() {
  const checked: Out2<string[]> = holder.list.queryOptions();
  const read = holder.list.queryOptions();
  const wrong: Out1<string[]> = read;
  return [checked, wrong];
}
