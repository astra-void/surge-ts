function get<T, K extends keyof T>(obj: T, key: K): T[K] {
  return obj[key];
}

// TS2304 only: an unresolved receiver type argument must not cascade into a
// spurious T[K] constraint or property error.
const bad = get<Missing, "id">({} as never, "id");
