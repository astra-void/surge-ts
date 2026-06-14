function get<T, K extends keyof T>(obj: T, key: K): T[K] {
  return obj[key];
}

type User = { id: string; age: number };

const a: string = get<User, "id">({ id: "a", age: 1 }, "id");
const b: number = get<User, "age">({ id: "a", age: 1 }, "age");
