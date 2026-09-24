interface NotGeneric { p: string; }
let a: Unknown<Arg1, Box<Arg2>>;
let b: NotGeneric<Arg3>;
let c: Array<Arg4>;
export {};
