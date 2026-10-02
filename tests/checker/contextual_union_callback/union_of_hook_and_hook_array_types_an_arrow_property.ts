interface Ctx { options: { body?: string } }
type Hook = (context: Ctx) => void;
interface Opts { onResponse?: Hook | Hook[] }
declare function request(url: string, opts?: Opts): void;
request("/x", {
onResponse: (ctx) => {
const body: string | undefined = ctx.options.body;
void body;
},
});
