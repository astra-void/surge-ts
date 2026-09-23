class B { m() {} }
class C extends B {
  m() { super; }
  o() { super?.m; }
  p() { (super); }
  q() { super.m(); }
}
export {};
