function f() {
type Local = { p: string };
return null as any as Local;
}
const bad: Local = { p: "x" };
export { f, bad };
