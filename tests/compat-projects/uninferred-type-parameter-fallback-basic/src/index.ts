interface Root<A, R = { a: A }> {
  r: R;
}

declare function bare<A>(a?: A): Root<A>;
declare function constrained<A extends { e?: unknown }>(a?: A): Root<A>;
declare function objectConstrained<A extends object>(): Root<A>;
declare function defaulted<A = string>(a?: A): Root<A>;

class Builder<C extends object> {
  create<O extends { e?: unknown }>(opts?: O): Root<C, { c: C; o: O }> {
    return null!;
  }
}

export const unconstrainedIsUnknown: number = bare().r;
export const constraintStandsIn: number = constrained().r;
export const defaultStandsIn: number = defaulted().r.a;
export const methodOnInstance: number = new Builder<{ id: 1 }>().create().r.o;

// An argument that supplies the parameter still infers it.
export const inferredFromArgument: { a: string } = bare("s").r;
export const inferredConstrained: number = constrained({ e: 1 }).r.a.e;

// `A extends object` with no candidate is `object`, which accepts any object.
export const objectAccepted: { a: object } = objectConstrained().r;
