// @surge-compare: messages
type Unwrap<T> = T extends (...args: any[]) => infer R ? Awaited<R> : T;
declare function make(): Promise<{ a: number }>;
declare const unwrapped: Unwrap<typeof make>;
export const missing = unwrapped.nope;
