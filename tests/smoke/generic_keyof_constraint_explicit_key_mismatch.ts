function get<T, K extends keyof T>(obj: T, key: K): T[K] {
  return obj[key];
}

type User = { id: string; age: number };

// TS2344: "missing" is not a key of User. tsc points the span at the type
// argument; the pinned span here is the call callee because string-literal
// type arguments do not carry a span in the parsed AST.
const bad = get<User, "missing">({ id: "a", age: 1 }, "missing");
