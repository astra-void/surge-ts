interface Wrapper<T> {
  value: T;
}

declare function unwrap<T>(wrapper: Wrapper<T>): T;

const value: number = unwrap({ value: "hello" });
