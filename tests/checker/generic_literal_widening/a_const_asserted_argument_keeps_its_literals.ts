// A `const` assertion is not a fresh literal, so its literal types survive.
declare function id<T>(v: T): T;
export const kept: 1 = id({ n: 1 } as const).n;
