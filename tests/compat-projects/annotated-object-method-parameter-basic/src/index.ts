interface Observer<T> {
  next(value: T): void;
}

interface Observable<T> {
  subscribe(observer: Observer<T>): () => void;
}

export function observable<T>(
  subscribe: (observer: Observer<T>) => void,
): Observable<T> {
  const self: Observable<T> = {
    subscribe(observer) {
      subscribe(observer);
      return () => {};
    },
  };
  return self;
}

interface Builder {
  input(input: string): Builder;
  use(fn: (x: number) => void): Builder;
  meta: (meta: { tag: string }) => Builder;
}

export function makeBuilder(): Builder {
  const builder: Builder = {
    input(input) {
      return makeBuilder().use(() => input.length);
    },
    use(fn) {
      fn(1);
      return makeBuilder();
    },
    meta(meta) {
      return makeBuilder().input(meta.tag);
    },
  };
  return builder;
}

type Mode = 'simple' | 'detailed';

export function pickMode(detailed: boolean): Mode {
  let mode: Mode = detailed ? 'detailed' : 'simple';
  if (mode === 'detailed') {
    mode = 'simple';
  }
  return mode;
}

export function stillReports(): Builder {
  const builder: Builder = {
    input(input) {
      return makeBuilder().use(() => input.missing);
    },
    use(fn) {
      fn('one');
      return makeBuilder();
    },
    meta(meta) {
      return makeBuilder().input(meta.tag);
    },
  };
  return builder;
}
