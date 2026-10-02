// Widening the contextual type to `T | undefined` must not cost a callback its
// parameter types — that would surface as TS7006 on `ctx` under noImplicitAny.
interface Ctx { body: string }
interface Props { on?: (context: Ctx) => void }
declare function take(props: Props): void;
take({
on: (ctx) => {
const body: string = ctx.body;
void body;
},
});
