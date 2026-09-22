class Shape {
  constructor(
    public name: string,
    public sides: number,
  ) {}
}

export class TooFew extends Shape {
  constructor() {
    super("few");
  }
}

export class WrongType extends Shape {
  constructor() {
    super("wrong", "three");
  }
}

export class TooMany extends Shape {
  constructor() {
    super("many", 3, true);
  }
}

export class JustRight extends Shape {
  constructor() {
    super("right", 4);
  }
}

class Plain {}

export class PlainChild extends Plain {
  constructor() {
    super(1);
  }
}

class Overloaded {
  constructor(id: number);
  constructor(first: string, last: string);
  constructor(a: number | string, b?: string) {
    void a;
    void b;
  }
}

export class ByName extends Overloaded {
  constructor() {
    super("ada", "lovelace");
  }
}

export class ByFlag extends Overloaded {
  constructor() {
    super(true);
  }
}

class Options {
  constructor(options: { depth: number; label?: string }) {
    void options;
  }
}

export class BadMember extends Options {
  constructor() {
    super({ depth: "deep" });
  }
}

export class ExcessMember extends Options {
  constructor() {
    super({ depth: 1, colour: "red" });
  }
}

export class NativeBase extends Error {
  constructor(message: string) {
    super(message);
  }
}
