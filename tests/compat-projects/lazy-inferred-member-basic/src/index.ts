import { prepareUrl } from './url';

declare function subject<T>(value: T): { get(): T; next(value: T): void };

class Connection {
  readonly observable = subject<Date | undefined>(undefined);

  get current() {
    return this.observable.get();
  }

  get label() {
    if (this.current) {
      return 'open';
    }
    return 'closed';
  }
}

declare const connection: Connection;
export const current: number = connection.current;
export const label: number = connection.label;

export async function open(): Promise<void> {
  const url: number = await prepareUrl({ url: 'x', params: true });
}
