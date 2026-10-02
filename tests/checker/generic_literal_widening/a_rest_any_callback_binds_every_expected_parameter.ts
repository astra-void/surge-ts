// A callback argument infers through a rest parameter's *element*:
// `(...args: any[]) => any` — vitest's `vi.fn()` — binds each expected
// parameter to `any`, as tsc does. Zipping positionally bound the first to
// `any[]` and the rest to nothing, which left `TVariables` at its `void`
// default and rejected every `mutate(1)`.
declare const anyFn: (...args: any[]) => any;
interface Opts<TData, TVariables> {
mutationFn?: (variables: TVariables) => Promise<TData>;
onSuccess?: (data: TData, variables: TVariables) => void;
}
declare function observe<TData = unknown, TVariables = void>(
options: Opts<TData, TVariables>,
): { mutate: (variables: TVariables) => Promise<TData> };
export function f() {
const observer = observe({ mutationFn: () => Promise.resolve('data'), onSuccess: anyFn });
observer.mutate(1);
}
