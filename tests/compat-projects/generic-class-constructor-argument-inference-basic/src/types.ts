export type MutationFunction<TData, TVariables> = (
  variables: TVariables,
) => Promise<TData>;

export interface MutationOptions<TData, TError, TVariables> {
  mutationFn?: MutationFunction<TData, TVariables>;
  onError?: (error: TError, variables: TVariables) => void;
}

export interface ObserverOptions<TData, TError, TVariables>
  extends MutationOptions<TData, TError, TVariables> {
  retry?: number;
}
