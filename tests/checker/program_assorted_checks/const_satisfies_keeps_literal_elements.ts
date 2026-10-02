// `x as const satisfies T` keeps the const-asserted literal element types
// (plain `infer_expression` would widen them to `string`).
type Device = "iphone" | "android" | "other";
const DEVICES = ["iphone", "android", "other"] as const satisfies Device[];
const first: Device = DEVICES[0];
