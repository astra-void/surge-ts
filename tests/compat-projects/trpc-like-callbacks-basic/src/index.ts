declare function query(fn: (opts: unknown) => unknown): unknown;

query(({ ctx, input }) => input);
