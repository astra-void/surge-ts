// An argument-derived candidate is authoritative: the contextual type only fills
// in what the arguments left unresolved, so a deliberately different annotation
// does not silently rewrite the inferred type argument.
// @noImplicitAny: true
declare function wrap<T>(value: T): T[];
const wrapped: string[] = wrap(1);
void wrapped;
