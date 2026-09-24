class Base {
  protected x = 0;
  protected m() {
    return 0;
  }
}
class Derived1 extends Base {
  read(base: Base, own: Derived1, sibling: Derived2, deeper: Derived3) {
    return [base.x, own.x, sibling.x, deeper.x, super.m(), this.x];
  }
  static readStatic(base: Base) {
    return base.x;
  }
}
class Derived2 extends Base {}
class Derived3 extends Derived1 {}

export function viaSibling<T extends Derived2>(this: T, other: Derived1) {
  return other.x;
}

export { Base, Derived1, Derived2, Derived3 };
