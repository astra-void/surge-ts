declare class Base {
  a: number;
  constructor(text: string);
  constructor(count: number);
}
declare class MixinOne {
  constructor(...args: any[]);
  p: number;
  static staticP: number;
}
declare class MixinTwo {
  constructor(...args: any[]);
  f(): number;
}

declare const MixedFirst: typeof MixinOne & typeof Base;
declare const MixedLast: typeof Base & typeof MixinOne;
declare const MixedThree: typeof MixinTwo & typeof MixinOne & typeof Base;
declare const OnlyMixins: typeof MixinOne & typeof MixinTwo;

export function instances() {
  const first = new MixedFirst("hello");
  const last = new MixedLast(42);
  const three = new MixedThree("hello");
  const mixins = new OnlyMixins();
  const wrongArgument = new MixedFirst(true);
  const missing = first.notThere;
  const asString: string = three.f();
  return [first.a, first.p, last.a, last.p, three.a, three.p, mixins.p, mixins.f(), MixedFirst.staticP, wrongArgument, missing, asString];
}
