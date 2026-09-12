import type { ObserverOptions } from './types';

class Observer<TData = unknown, TError = Error, TVariables = void> {
  constructor(public options: ObserverOptions<TData, TError, TVariables>) {}
  mutate(variables: TVariables): void {
    void variables;
  }
}

class Box<T = void> {
  constructor(public value: T) {}
  take(value: T): void {
    void value;
  }
}

export function run(): void {
  const observer = new Observer({
    mutationFn: (text: string) => Promise.resolve(text.length),
  });
  observer.mutate('input');

  const box = new Box('text');
  box.take('input');

  const empty = new Observer({ retry: 1 });
  empty.mutate();
}
