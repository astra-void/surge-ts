// The hook-option shape `Hook | Hook[]` (ofetch's `FetchHooks`, tRPC's option
// bags): a callback written against it takes its parameter types from the
// union's single callable member, both as a method shorthand and as an arrow.
interface Ctx { options: { body?: string } }
type MaybeArray<T> = T | T[];
type Hook = (context: Ctx) => void;
interface Opts { onResponse?: MaybeArray<Hook> }
declare function request(url: string, opts?: Opts): void;
request("/x", {
onResponse(ctx) {
const body: string | undefined = ctx.options.body;
void body;
},
});
