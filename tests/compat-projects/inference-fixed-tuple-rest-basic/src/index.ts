type Procedure = (...args: any[]) => any;
interface MockInstance<T extends Procedure = Procedure> {
  mock: { calls: Parameters<T>[] };
}
interface Mock<T extends Procedure = Procedure> extends MockInstance<T> {
  (...args: Parameters<T>): ReturnType<T>;
}
declare function fn<T extends Procedure = Procedure>(impl?: T): Mock<T>;
type MutationFunction<TData = unknown, TVariables = unknown> = (
  variables: TVariables,
  context: { a: 1 },
) => Promise<TData>;
interface MObsOptions<TData, TError, TVariables> {
  mutationFn?: MutationFunction<TData, TVariables>;
}
declare class MObserver<TData = unknown, TError = Error, TVariables = void> {
  constructor(client: {}, options: MObsOptions<TData, TError, TVariables>);
  mutate(variables: TVariables, options?: {}): Promise<TData>;
}
const mutationFn = fn(() => Promise.resolve('data'));
const observer = new MObserver({}, { mutationFn });
observer.mutate();
const y: 0 = observer;
const y2: 0 = mutationFn;
