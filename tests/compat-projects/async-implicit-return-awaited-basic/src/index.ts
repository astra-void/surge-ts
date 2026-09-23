interface Thenable<T> {
  then(onfulfilled: (value: T) => void): void;
}
declare const pending: Thenable<void> | null;
declare const flag: boolean;

export async function open() {
  if (flag) {
    return pending;
  }
}

export async function count() {
  if (flag) {
    return 1;
  }
}
