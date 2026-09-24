class Base {
  protected shared() {}
  protected static create() {}
}
class Left extends Base {
  protected left() {}
}
class Right extends Base {
  protected right() {}
}

export function throughConstraint<T extends Base>(this: T, other: Left) {
  other.shared();
  other.left();
}

export function throughClass(this: Left, other: Left) {
  other.shared();
  other.left();
  Base.create();
}

export function throughUnrelated(this: Right, other: Left) {
  other.left();
}

export class Host {
  protected hosted() {}
  visit(this: Left, other: Left) {
    other.left();
  }
  nested(this: Left, other: Left) {
    function inner() {
      other.left();
    }
    const arrow = () => other.left();
    return [inner, arrow];
  }
}
