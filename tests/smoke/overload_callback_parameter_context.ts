interface Effects<O> {
  _o: O;
}

declare class Schema<Output = any> {
  refine<Refined extends Output>(
    check: (arg: Output) => arg is Refined,
    message?: string
  ): Effects<Refined>;
  refine(check: (arg: Output) => unknown, message?: string): Effects<Output>;
}

declare const schema: Schema<Date>;

export const refined = schema.refine((d) => d.toISOString());
