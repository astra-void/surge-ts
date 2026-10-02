// A `function` declaration nested in another function keeps its collected
// signature when hoisted into scope, so a sibling closure's predicate guard
// still narrows (the unnamed settings-page shape).
type Device = "iphone" | "android" | "other";
function outer(m: string | undefined): Device {
function isDevice(value: string): value is Device {
return value === "iphone";
}
const get = (): Device => {
if (m && isDevice(m)) return m;
return "other";
};
return get();
}
