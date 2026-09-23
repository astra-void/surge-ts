type T<const U> = U;
interface I<const T> { x: T }
class C<const T> {}
function f<const T>(x: T) { return x; }
declare const g: <const T>(x: T) => T;
interface J { m<const T>(x: T): T; n: <const T>(x: T) => T; new <const T>(x: T): J; <const T>(x: T): T }
class D { m<const T>(x: T) { return x; } }
const arrow = <const T,>(x: T) => x;
type Ctor = new <const T>(x: T) => T;
export {};
