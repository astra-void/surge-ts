// `class D extends Parent {}` over a value: the base type comes from the
// value's construct signature, so nothing is unresolved and the inherited
// members are visible.
declare const Base: new (..._args: any[]) => { z: number };
export function f() {
const Parent = Base;
class Definition extends Parent {}
return new Definition().z;
}
