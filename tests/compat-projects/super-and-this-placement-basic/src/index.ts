export {};

class Base {
  m() {
    return 1;
  }
}
class NotDerived {
  constructor() {
    super();
  }
  m() {
    return super.m();
  }
}
class Derived extends Base {
  constructor() {
    super();
    const f = function () {
      super();
    };
  }
  n() {
    super();
    return super.m();
  }
}
function outside() {
  super.x;
}
class ComputedSuper extends Base {
  [super.m()]() {}
}
const literal = {
  m() {
    return super.toString();
  },
};

class Computed {
  [(this as any).key]() {}
}
namespace Holder {
  const self = this;
}
enum Values {
  A = (this as any),
}
function noThisParameter() {
  return this;
}
function withThisParameter(this: string) {
  return this;
}
function arrowCapture() {
  const a = () => this;
}
const methodThis = {
  m() {
    return this;
  },
};
class Owner {
  m() {
    function inner() {
      return this;
    }
    return this;
  }
}
