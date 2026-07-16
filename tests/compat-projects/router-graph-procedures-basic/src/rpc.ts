export interface Procedure<TInput, TOutput> {
  (input: TInput): Promise<TOutput>;
  readonly _input: TInput;
  readonly _output: TOutput;
}

export declare function procedure<TInput, TOutput>(): Procedure<TInput, TOutput>;

export declare function router<T extends Record<string, unknown>>(
  routes: T,
): T & { readonly _router: true };

export type InputOf<P> = P extends Procedure<infer I, infer O> ? I : never;
export type OutputOf<P> = P extends Procedure<infer I, infer O> ? O : never;
