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

interface QueryProcedure {
  (opts: { path: string }): Promise<string>;
  _def: { type: 'query' };
}
interface MutationProcedure {
  (opts: { path: string }): Promise<string>;
  _def: { type: 'mutation' };
}
type AnyProcedure = QueryProcedure | MutationProcedure;

declare function procedureAt(path: string): AnyProcedure | undefined;

export function callProcedure(path: string): Promise<string> {
  const procedure = procedureAt(path);
  if (!procedure) {
    throw new Error('not found');
  }
  return procedure({ path });
}

declare const holder: {
  run: QueryProcedure | MutationProcedure;
  maybeRun?: QueryProcedure | ((opts: { path: string }) => Promise<string>);
};

export function callThroughAProperty(path: string): Promise<string> {
  return holder.run({ path });
}

export function callThroughANarrowedProperty(path: string): Promise<string> {
  if (typeof holder.maybeRun === 'function') {
    return holder.maybeRun({ path });
  }
  return Promise.resolve('');
}

type EnabledFn = (opts: { direction: 'up' | 'down' }) => boolean;

declare const enabledOption: EnabledFn | undefined;

export function callAcrossArities(): boolean {
  const enabled = enabledOption ?? (() => true);
  return enabled({ direction: 'up' });
}
