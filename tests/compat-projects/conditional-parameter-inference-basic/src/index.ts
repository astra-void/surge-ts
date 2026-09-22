type ValidateShape<TActualShape, TExpectedShape> = TActualShape extends TExpectedShape
  ? Exclude<keyof TActualShape, keyof TExpectedShape> extends never
    ? TActualShape
    : TExpectedShape
  : never;

interface Options {
  transformer?: unknown;
  isServer?: boolean;
}

declare function create<TOptions extends Options>(
  opts?: ValidateShape<TOptions, Options>,
): { opts: TOptions; transformed: undefined extends TOptions['transformer'] ? false : true };

const withTransformer = create({ transformer: 1 });
const optsAsNumber: number = withTransformer.opts;
const transformedAsString: string = withTransformer.transformed;

const bare = create({ isServer: true });
const bareTransformed: string = bare.transformed;

declare function pick<T>(value: T extends string ? T : never): T;
const picked = pick('a');
const pickedAsNumber: number = picked;
