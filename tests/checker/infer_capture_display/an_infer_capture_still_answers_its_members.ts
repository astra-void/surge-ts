// The member still resolves through the capture: the binding was always correct,
// and the display fix must not change what the type answers.
// @surge-compare: messages
type Unwrap<T> = T extends (...args: any[]) => infer R ? Awaited<R> : T;
declare function make(): Promise<{ a: number }>;
declare const unwrapped: Unwrap<typeof make>;
export const wrong: string = unwrapped.a;
