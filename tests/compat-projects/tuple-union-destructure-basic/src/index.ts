interface RequestError {
  code: string;
}
interface RequestInfo {
  isBatchCall: boolean;
  calls: string[];
}

type ResultTuple<T> = [undefined, T] | [RequestError, undefined];

declare const infoTuple: ResultTuple<RequestInfo>;
declare function useInfo(info: RequestInfo): void;

export function afterThrowingTheError(): number {
  const [infoError, info] = infoTuple;
  if (infoError) {
    throw infoError;
  }
  useInfo(info);
  return info.calls.length;
}

export function theOtherSideNarrowsToo(): string {
  const [infoError, info] = infoTuple;
  if (!infoError) {
    return String(info.isBatchCall);
  }
  return infoError.code;
}

export function elementTypeIsRealWithoutAGuard(): boolean | undefined {
  const [, info] = infoTuple;
  return info?.isBatchCall;
}

export function unguardedElementKeepsItsUndefined(): void {
  const [, info] = infoTuple;
  const stillOptional: RequestInfo = info;
  void stillOptional;
}
