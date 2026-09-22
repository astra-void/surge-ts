abstract class Formatter {
  constructor(
    protected format: (value: number) => string,
    ...flags: boolean[]
  ) {
    void flags;
  }
}

class Middle extends Formatter {}

export const contextual = new Middle((value) => value.toFixed(2), true);
export const badReturn = new Middle((value) => value);
export const badRest = new Middle((value) => String(value), 1);
export const missing = new Middle();

export class Leaf extends Middle {
  constructor() {
    super((value) => value.toFixed(), false);
  }
}

export class BadLeaf extends Middle {
  constructor() {
    super((value) => value);
  }
}

class Pair {
  constructor(left: number);
  constructor(left: string, right: string);
  constructor(left: number | string, right?: string) {
    void left;
    void right;
  }
}

class PairChild extends Pair {}

export const one = new PairChild(1);
export const two = new PairChild("a", "b");
export const wrong = new PairChild(true);
export const instance: PairChild = one;
export const notBase: { missing: number } = new PairChild(2);
