type Box<T> = { value: T };
type Outer<T> = { inner: Box<T> };

let outer: Outer<string> = { inner: { value: "ok" } };
