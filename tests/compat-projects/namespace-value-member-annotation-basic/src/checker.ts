export interface Checker<T> {
  check(value: unknown): value is T;
  name: string;
}
