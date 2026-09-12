export type Record1<T> = { [K in keyof T]: T[K] };

export type Caller<T> = (
  ctx: number,
  options?: { onError?: (error: string) => void },
) => Record1<T>;

export interface Factory {
  create(): Caller<{ id: number }>;
  createFor<T>(value: T): Caller<T>;
}
