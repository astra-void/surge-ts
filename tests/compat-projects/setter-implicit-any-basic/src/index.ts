class C { set x(v) {} }
class D { set y(v) {} get y() { return 1; } }
class E { set z(v) {} get z(): number { return 1; } }
class F { private set w(v) {} }
class G { static set s(v) {} set "lit"(v) {} }
class H { set typed(v: number) {} }
const o = { set p(v) {} };
const q = { set p(v) {}, get p() { return 1; } };
type T = { set t(v); };
export {}
