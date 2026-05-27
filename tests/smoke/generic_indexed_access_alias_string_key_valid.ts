type Prop<T> = T["value"];
type Value = Prop<{ value: string }>;

let ok: Value = "ok";
