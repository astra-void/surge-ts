export function h(c: boolean) {
if (c) {
type Inner = { q: string };
const i: Inner = { q: "a" };
return i.q;
}
return "";
}
