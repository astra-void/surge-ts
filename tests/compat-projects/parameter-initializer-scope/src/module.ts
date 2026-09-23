export {};

const label: string = "";

// The body's own declarations are not in scope in a parameter initializer;
// every parameter of the list is, itself and the ones after it included.
export function outer(value = label) { var label: number = 2; return value.length; }
export function earlier(first: string, second = first) { return second.length; }
export function deferred(read = () => later, later = 1) { const result: number = read(); return result; }
export function laterAnnotation(value: typeof later, later: number) { const result: number = value; return result; }
export function annotatedSelf(value: number = value) { return value; }
export const arrow = (first = second, second = "") => first;

// A later parameter is read at its own type.
export function laterMember(size = later.length, later = 1) { return size; }
export function deferredWrong(read = () => later, later = 1) { const result: string = read(); return result; }

// An initializer reading its own parameter eagerly is circular: `any`, and an
// implicit one under `noImplicitAny`.
export function circular(value = value) { return value; }
export function cycle(first = second, second = first) { return [first, second]; }
export function deferredSelf(read = () => read) { return read; }
