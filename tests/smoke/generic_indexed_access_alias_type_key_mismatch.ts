type PickValue<T, K> = T[K];
type Value = PickValue<{ id: string; age: number }, "id">;

let bad: Value = 123; // TS2322
