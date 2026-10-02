// The heritage fallback is heritage-only: a value named in an ordinary
// annotation position is still reported.
declare const Base: new (..._args: any[]) => { z: number };
const Parent = Base;
declare let x: Parent;
export { x };
