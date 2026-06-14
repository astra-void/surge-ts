function get<T, K extends keyof T>(obj: T, key: K): T[K] {
  return obj[key];
}

type User = { id: string; age: number };

// TS2344: the "missing" member of the union is not a key of User.
const bad = get<User, "id" | "missing">({ id: "a", age: 1 }, "id");
