declare function decorate(target: Function): void;

@decorate
export class Decorated {
  static base = 1;
  static direct = this.base + 1;
  static viaArrow = () => this.base;
  static viaFunction = function () {
    return this;
  };
  static { this.base; }
  instance = this;
}

export class Plain {
  static base = 1;
  static direct = this.base + 1;
  property = function () {
    return this;
  };
  typed: () => unknown = function () {
    return this;
  };
  method() {
    return function () {
      return this;
    };
  }
}

export const make = function () {
  return function () {
    return this;
  };
};
