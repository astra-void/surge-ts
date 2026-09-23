class B { m<T>() {} }
class C extends B {
  constructor() { super<string>(); }
  n() { super<string>.m(); }
  ok() { super.m<string>(); }
}
export {};
