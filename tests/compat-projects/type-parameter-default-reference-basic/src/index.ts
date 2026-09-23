type A<T = U, U = string> = [T, U];
function f<T = U, U = number>(a: T, b: U) {}
type Self<T = T> = T;
type Ok<T, U = T> = [T, U];
type Shadow<T = <U>() => U, U = string> = [T, U];
type Cond<T = U extends infer U ? U : never, U = string> = [T, U];
type Mapped<T = { [U in "a"]: U }, U = string> = [T, U];
interface I<T extends U, U = string> { t: T; u: U }
class C<T = U[], U = number> { t!: T; u!: U }
type Later<T = Box<U>, U = string> = [T, U]; type Box<X> = { x: X };
export {}
