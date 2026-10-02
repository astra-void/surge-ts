// `(...args: infer P)` captures the whole parameter tuple, not the parameter at
// that position — what makes `ConstructorParameters<T>[0]` yield a constructor's
// first argument. The resulting tuple-typed rest parameter is then a positional
// parameter list at both call sites and in assignability.
type FirstCtorArg<T extends new (..._a: any) => any> = ConstructorParameters<T>[0];
interface Ctor { new (cmd: [key: string, unix: number], opts?: number): object }
declare const call: (...args: FirstCtorArg<Ctor>) => void;
call("a", 1);
export const plain: (key: string, unix: number) => void = call;
