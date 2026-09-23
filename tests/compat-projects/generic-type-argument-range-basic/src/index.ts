interface Box<T, U = string> { t: T; u: U }
type Alias<A, B = number, C = boolean> = [A, B, C];
type One<T> = T[];
let a: Box;
let b: Box<number, string, boolean>;
let c: Alias;
let d: Alias<1, 2, 3, 4>;
let e: One;
let f: Box<number>;
let g: Alias<1>;
class Gen<T, U = T> { t!: T; u!: U }
let h: Gen;
export {};
