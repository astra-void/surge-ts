interface Failure<TShape> {
  shape: TShape;
  message: string;
}

type Mutation =
  | { status: 'idle'; error: null }
  | { status: 'error'; error: Failure<{ code: number }> };

declare const mutation: Mutation;

if (mutation.error?.shape) {
  mutation.error.shape.code;
  mutation.error.message;
}

declare const nested: { inner: { error: Failure<string> | null } | undefined };

if (nested.inner?.error?.shape) {
  nested.inner.error.shape.length;
}

if (mutation.error?.shape.code) {
  mutation.error.shape.code.toFixed();
}

if (mutation.error.shape) {
  mutation.error.message;
}
