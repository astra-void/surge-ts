export type Box<T> = { value: T };

export interface StoreApi<TState> {
  getState: () => TState;
}
