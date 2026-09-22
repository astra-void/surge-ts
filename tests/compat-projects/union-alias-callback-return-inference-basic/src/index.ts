type MaybePromise<T> = Promise<T> | T;
type OrUndefined<T> = T | undefined;

declare function resolve<O>(resolver: () => MaybePromise<O>): O;
declare function optional<O>(read: () => OrUndefined<O>): O;
declare function direct<O>(value: MaybePromise<O>): O;

export const fromString: number = resolve(() => "s");
export const fromAsync: number = resolve(async () => "s");
export const fromUnionAlias: number = optional(() => "s");
export const fromValue: number = direct("s");

// The awaited shape and a matching result stay accepted.
export const fromPromise: number = resolve(() => Promise.resolve(1));
export const fromNumber: number = resolve(() => 1);
