export async function count(): number {
  return 1;
}

export const label = async (): string => "x";

export class Worker {
  async ready(): boolean {
    return true;
  }
  async done(): Promise<void> {}
}

export const handlers = {
  async load(): number[] {
    return [];
  },
};

type Deferred<T> = Promise<T>;
export async function viaAlias(): Deferred<number> {
  return 1;
}

export async function* stream(): AsyncGenerator<number> {
  yield 1;
}
