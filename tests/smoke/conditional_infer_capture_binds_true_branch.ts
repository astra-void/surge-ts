interface Box<T = unknown> {
  v: T;
}

type Unwrap<S> = S extends Box<infer T> ? T : never;

const value: number = "s" as Unwrap<Box<string>>;
