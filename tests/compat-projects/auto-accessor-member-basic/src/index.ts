export class Counter {
  accessor count: number = 0;
  accessor label = "counter";
  static accessor instances: number = 0;
  accessor inferred;
  static accessor untyped;

  constructor() {
    this.inferred = 1;
  }

  bump() {
    this.count += 1;
    Counter.instances += 1;
    const wrong: string = this.count;
    this.label = 3;
    return [wrong, this.missing];
  }
}

export const counter = new Counter();
export const read: number = counter.count;
export const readWrong: boolean = counter.label;

export class Inferred {
  viaElement;
  0;
  static viaStaticBlock;
  static accessor autoStatic;
  static neverAssigned;

  constructor(seed: number) {
    this["viaElement"] = seed;
    this[0] = seed;
  }

  static {
    this.viaStaticBlock = 1;
    this.autoStatic = 2;
    Counter.instances = 3;
  }
}
