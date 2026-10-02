export function f() {
class Bar { x = 1 }
interface Baz { y: number }
const z: Baz = { y: 2 };
return new Bar().x + z.y;
}
