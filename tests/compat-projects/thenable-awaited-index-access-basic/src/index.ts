export abstract class QueryPromise<T> implements Promise<T> {
  [Symbol.toStringTag] = 'QueryPromise';
  abstract execute(): Promise<T>;
  catch<R = never>(onRejected?: ((reason: any) => R | PromiseLike<R>) | null): Promise<T | R> {
    return this.then(undefined, onRejected);
  }
  finally(onFinally?: (() => void) | null): Promise<T> {
    return this.then(
      (value) => value,
      (reason) => {
        onFinally?.();
        throw reason;
      },
    );
  }
  then<R1 = T, R2 = never>(
    onFulfilled?: ((value: T) => R1 | PromiseLike<R1>) | undefined | null,
    onRejected?: ((reason: any) => R2 | PromiseLike<R2>) | undefined | null,
  ): Promise<R1 | R2> {
    return this.execute().then(onFulfilled, onRejected);
  }
}

export class Raw<TResult> extends QueryPromise<TResult> {
  constructor(public execute: () => Promise<TResult>) {
    super();
  }
}

declare function runQuery(): Raw<{ id: number }[]>;

export async function firstRow(): Promise<{ id: number } | undefined> {
  const rows = await runQuery();
  return rows[0];
}

export async function aPlainPromiseStillIndexes(): Promise<number> {
  const values = await Promise.resolve([1, 2, 3]);
  return values[0]!;
}

export async function theElementIsStillPossiblyAbsent(): Promise<{ id: number }> {
  const rows = await runQuery();
  return rows[0];
}
