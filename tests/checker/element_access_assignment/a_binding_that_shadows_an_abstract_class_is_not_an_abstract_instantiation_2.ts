// tsc resolves the *value* before deciding an instantiation is abstract; the
// name-keyed type lookup reported every `new Class()` in a function whose
// parameter shadowed an abstract class of that name. zod declares
// `abstract class Class` and passes a `Class` parameter into the same module.
export abstract class Class { constructor(public n: number) {} }
export function g() { return new Class(1); }
