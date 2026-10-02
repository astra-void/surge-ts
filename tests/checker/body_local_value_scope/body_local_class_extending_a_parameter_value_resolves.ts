declare const Base: new (..._args: any[]) => { z: number };
export function f(Parent: typeof Base) {
class Definition extends Parent {}
return new Definition().z;
}
