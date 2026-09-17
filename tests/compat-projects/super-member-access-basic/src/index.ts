class Base {
  method(a: number): number {
    return a;
  }
  get label(): string {
    return "base";
  }
  static create(n: number): number {
    return n;
  }
}

export class Derived extends Base {
  wrongArgument() {
    return super.method("x");
  }
  missing() {
    return super.nothing;
  }
  typed() {
    const text: number = super.label;
    return text;
  }
  static make() {
    return super.create("y");
  }
  field = () => super.method(1);
  constructor() {
    super();
  }
}

export const literal = {
  m() {
    return super.toString();
  },
};
