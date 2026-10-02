class Parent { constructor(..._a: any[]) {} }
export function f() {
class Definition extends Parent {}
return new Definition();
}
