interface Wrap<T> { v: T; }
export function outer() {
type TError = Wrap<string>;
const a: TError = null as any;
function inner(o?: TError): TError { return o!; }
return { a, inner };
}
