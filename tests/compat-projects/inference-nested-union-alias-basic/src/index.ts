type MaybePromise<T> = T | Promise<T>;
type Setup<C> = (context: {}) => MaybePromise<void | C>;

declare function setup<C extends Record<string, unknown>>(fn: Setup<C>): C;
const created = setup(() => ({ count: 1 }));
export const count: string = created.count;

declare function direct<C>(fn: () => MaybePromise<void | C>): C;
export const label: number = direct(() => "ready");
