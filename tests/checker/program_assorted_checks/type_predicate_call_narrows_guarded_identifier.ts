// A user-defined type predicate (`value is T`) narrows its bare-identifier
// argument in the guarded branch, composing with `&&` (`m && isDevice(m)`).
type Device = "iphone" | "android" | "other";
function isDevice(value: string): value is Device {
return value === "iphone";
}
function pick(m: string | undefined): Device {
if (m && isDevice(m)) return m;
return "other";
}
