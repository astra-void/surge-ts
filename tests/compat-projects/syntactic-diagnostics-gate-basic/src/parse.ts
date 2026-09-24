declare function identity<T>(value: T): T;
export const name = identity<string>.name;
