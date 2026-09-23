function outer() {
  class C {
    x = arguments;
    y = () => arguments;
    z = function () { return arguments; };
    static s = arguments;
    static { arguments; }
    static { const f = () => arguments; }
    m() { return arguments; }
  }
  return C;
}
class E { m() { class F { x = arguments; } return F; } }
export {};
