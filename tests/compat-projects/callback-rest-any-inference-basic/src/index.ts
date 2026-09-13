interface Mock { (...args: any[]): any; calls: number }
declare const mock: Mock;
declare const anyFn: (...args: any[]) => any;

interface Options<TData, TVariables> {
  mutationFn?: (variables: TVariables) => Promise<TData>;
  onSuccess?: (data: TData, variables: TVariables) => void;
}

declare class Observer<TData = unknown, TVariables = void> {
  constructor(options: Options<TData, TVariables>);
  mutate(variables: TVariables): Promise<TData>;
}

export function mockedCallback(): void {
  const observer = new Observer({ mutationFn: () => Promise.resolve('data'), onSuccess: mock });
  observer.mutate(1);
}

export function restAnyCallback(): void {
  const observer = new Observer({ mutationFn: () => Promise.resolve('data'), onSuccess: anyFn });
  observer.mutate(1);
}

export function noSourceKeepsTheDefault(): void {
  const observer = new Observer({ mutationFn: () => Promise.resolve('data') });
  // @ts-expect-error TVariables defaults to void, so a number is rejected
  observer.mutate(1);
}

export function annotatedParameterWins(): void {
  const observer = new Observer({ mutationFn: (count: number) => Promise.resolve(String(count)), onSuccess: anyFn });
  // @ts-expect-error TVariables is number, from the annotated parameter
  observer.mutate('wrong');
}
