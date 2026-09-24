declare function identity<T>(value: T): T;

export const lone = identity<?>;
export const nullable: string? = "value";
export const mismatch: number = "text";
