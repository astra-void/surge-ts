interface Options {
  readonly target: string;
  readonly libraryOptions?: Record<string, unknown> | undefined;
}
interface Converter {
  readonly input: (options: Options) => Record<string, unknown>;
}
interface Standard {
  readonly "~standard": { readonly jsonSchema: Converter };
}

type MethodParams = Parameters<Standard["~standard"]["jsonSchema"]["input"]>[0];

export function describe(params?: MethodParams): string {
  const { libraryOptions, target } = params ?? {};
  void libraryOptions;
  return String(target);
}
