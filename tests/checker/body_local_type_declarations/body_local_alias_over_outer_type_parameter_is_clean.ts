// A body-local alias may name the enclosing function's type parameters. They
// are not declarations, so they are seeded as degradation placeholders rather
// than reported as unresolved names.
interface ClientError<T> { code: string; router: T; }
export function makeHooks<TRouter>() {
type TError = ClientError<TRouter>;
const e: TError = null as any;
function use(x: TError): TError { return x; }
return { e, use };
}
