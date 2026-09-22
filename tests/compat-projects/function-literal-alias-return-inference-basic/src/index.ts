declare const unsetMarker: unique symbol;
type UnsetMarker = typeof unsetMarker;
type DefaultValue<TValue, TFallback> = TValue extends UnsetMarker ? TFallback : TValue;
type MaybePromise<T> = Promise<T> | T;
type Resolver<TOut, $Output> = (opts: { ctx: object }) => MaybePromise<DefaultValue<TOut, $Output>>;

interface Builder<TOut> {
  run<$Output>(resolver: Resolver<TOut, $Output>): { output: DefaultValue<TOut, $Output> };
}
declare const builder: Builder<UnsetMarker>;

const fromLiteral = builder.run(() => 1);
export const output: string = fromLiteral.output;

const fromBlock = builder.run(({ ctx }) => {
  const seen = ctx;
  return [seen];
});
export const list: string = fromBlock.output;
