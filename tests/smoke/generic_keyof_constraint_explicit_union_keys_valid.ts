function get<T, K extends keyof T>(obj: T, key: K): T[K] {
  return obj[key];
}

type User = { id: string; age: number };

const value = get<User, "id" | "age">({ id: "a", age: 1 }, "id");
