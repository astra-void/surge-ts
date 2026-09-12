declare const stamps: Array<number>;
declare const keys: ReadonlyArray<string>;
declare function maybeWork(): Promise<void> | undefined;
declare function report(error: unknown): void;

export const spread = stamps[1]! - stamps[0]!;
export const first = keys[0] === 'A';
export const unchecked: string | undefined = keys[0];

export function continueOptionalPromise() {
  const result = maybeWork();
  result?.catch(report);
  result?.then(() => undefined)?.finally(() => undefined);
}

interface QueryOptions {
  refetchOnMount?: boolean | 'always';
  refetchOnWindowFocus?: boolean | 'always';
}

export function siblingParameterTypeof(
  options: QueryOptions,
  field: (typeof options)['refetchOnMount'] & (typeof options)['refetchOnWindowFocus'],
): (typeof options)['refetchOnMount'] {
  return field ?? options.refetchOnMount;
}

export const wrongElement: number = keys[0];
