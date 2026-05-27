type PickValue<T, K> = T[K];
type Value = PickValue<{ id: string; age: number }, "id">;

let ok: Value = "ok";
