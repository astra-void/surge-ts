export type Decorate<T> = {
  [K in keyof T]: T[K] extends infer V ? (V extends number ? { value: V } : Decorate<V>) : never;
};
export declare function decorate<T>(): Decorate<T>;
