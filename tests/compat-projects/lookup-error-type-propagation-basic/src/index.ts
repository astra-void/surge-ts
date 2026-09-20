declare const client: { $on(): void };

interface QueryProcedure<TDef> {
  _def: TDef;
}
interface ProcedureBuilder {
  query<$Output>(
    resolver: () => $Output | Promise<$Output>,
  ): QueryProcedure<{ input: void; output: $Output }>;
}
declare const procedure: ProcedureBuilder;

export const all = procedure.query(() => {
  return client.task.findMany({});
});

const rows = client.task.findMany({});
rows.map((row) => row.id);
all._def.output.map((row) => row.id);

declare const query: { data?: typeof all._def.output };
export const open = query.data?.filter((row) => !row.completed).length ?? 0;

type Output<T> = T extends QueryProcedure<infer D extends { output: unknown }> ? D['output'] : never;
declare function getData(): Output<typeof all> | undefined;
const data = getData();
if (data) data.map((row) => row.id);
const list = data ?? [];
list.map((row) => row.id);

const whole: never = all;
export { whole };
