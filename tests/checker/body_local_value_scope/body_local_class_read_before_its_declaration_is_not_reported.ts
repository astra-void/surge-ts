// The class's static type is built at its own statement position, so a read
// from an earlier closure sees a reserved placeholder rather than nothing.
export function f() {
const make = () => new C();
class C { x = 1 }
return make().x;
}
