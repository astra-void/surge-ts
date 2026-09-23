const enum E { A = 1, B }
const a = E.A;
const b = E["A"];
const c = E[`A`];
const e = E[`${"A"}`];
const key = "A";
const f = E[key];
const g = E;
const h = (E).A;
const i = typeof E;
function take(x: unknown) {}
take(E);
let t: typeof E;
let m: E.A;
export { E };
export default E;
