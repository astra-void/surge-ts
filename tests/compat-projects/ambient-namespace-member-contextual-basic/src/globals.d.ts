declare namespace hostruntime {
  function streamifyResponse<TEvent = any, TResult = void>(
    handler: (event: TEvent) => TResult | Promise<TResult>,
  ): (event: TEvent) => TResult | Promise<TResult>;
  function onError(handler: (error: unknown) => void): void;
}
