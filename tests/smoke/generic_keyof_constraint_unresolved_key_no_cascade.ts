function get<T, K extends keyof T>(obj: T, key: K): T[K] {
  return obj[key];
}

type User = { id: string; age: number };

// TS2304 only: an unresolved key type argument must not cascade into a spurious
// T[K] indexing error.
const bad = get<User, MissingKey>({ id: "a", age: 1 }, "id");
