export {};

class Parameter {
  a = x;
  b!: typeof x;
  c = null as unknown as typeof x;
  constructor(x: number) {}
}

class ParameterProperty {
  a = x;
  b!: typeof x;
  constructor(public x: number) {}
}

class GenericParameterProperty<T> {
  a = this.x;
  b = x;
  constructor(public x: T) {}
}

class BodyLocals {
  a = y;
  b = z;
  c = f;
  constructor() {
    let y = 1;
    if (y) {
      var z = 2;
    }
    function f() {}
  }
}

class StaticMember {
  static s = 1;
  a = s;
  constructor(s: number) {}
}

class Accepted {
  a = this.x;
  b: typeof this.x = 0;
  c = () => (x: number) => x;
  constructor(public x: number) {}
}

class BlockScoped {
  a = q;
  constructor() {
    {
      let q = 1;
    }
  }
}

class StaticProperty {
  static a = x;
  constructor(x: number) {}
}
