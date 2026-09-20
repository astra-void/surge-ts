type Ctor<T = object> = new (...args: any[]) => T;

function Timestamped<B extends Ctor>(Base: B) {
  return class extends Base {
    timestamp = 1;
  };
}

class Root {
  r = 1;
  constructor(public label: string) {}
}

export class WithMixin extends Timestamped(Root) {
  own = true;

  describe(): string {
    return this.label + this.timestamp + this.r;
  }
}

export const mixed = new WithMixin("x");
export const inherited = [mixed.timestamp, mixed.r, mixed.label];
export const ownWrong: string = mixed.own;

export class FromConditional extends (Math.random() > 0.5 ? Root : Root) {
  z = 1;
}

export const viaConditional = new FromConditional("a").r;

class Plain {
  p = 1;
}

export const stillClosed = new Plain().nope;
