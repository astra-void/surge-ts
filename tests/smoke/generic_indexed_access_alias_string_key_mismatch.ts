type Prop<T> = T["value"];
type Value = Prop<{ value: string }>;

let bad: Value = 123; // TS2322
