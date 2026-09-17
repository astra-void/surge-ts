export {}
declare class Ambient { m: string }

class Exempt {
  withInit: string = "x";
  optional?: string;
  asserted!: string;
  maybeUndef: string | undefined;
  anyProp: any;
  unknownProp: unknown;
  static statik: string;
  declare declared: string;
}
abstract class AbstractHolder { abstract abs: string }
class ParamProps { constructor(public a: string, private b: number, readonly c: boolean) {} }
class AccessorsAndMethods {
  get g(): string { return "" }
  set s(v: string) {}
  m(): void {}
}

class BothBranches {
  v: string;
  constructor(cond: boolean) {
    if (cond) { this.v = "a" } else { this.v = "b" }
  }
}
class EarlyReturn {
  v: string;
  constructor(cond: boolean) {
    if (cond) { this.v = "a"; return }
    this.v = "b";
  }
}
class ThrowPath {
  v: string;
  constructor(cond: boolean) {
    if (!cond) { throw new Error() }
    this.v = "ok";
  }
}
class FromParameter {
  v: string;
  constructor(p: string) { this.v = p }
}
class CtorOverloads {
  v: string;
  constructor(a: string);
  constructor(a: number);
  constructor(a: string | number) { this.v = String(a) }
}

class NoCtor { bad: string }
class ReadonlyStillCounts { readonly ro: string }
class AbstractConcreteMember { concrete: string }
class PartiallyAssigned {
  assigned: string;
  notAssigned: string;
  constructor() { this.assigned = "a" }
}
class OnlyThen {
  v: string;
  constructor(cond: boolean) { if (cond) { this.v = "a" } }
}
class InLoop {
  v: string;
  constructor() { for (let i = 0; i < 1; i++) { this.v = "a" } }
}
class ViaHelper {
  v: string;
  constructor() { this.init() }
  init() { this.v = "h" }
}
