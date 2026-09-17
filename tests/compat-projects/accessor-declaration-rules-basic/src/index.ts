export abstract class A {
  get implicit() { }
  get throws(): number { throw new Error(); }
  get nested() { const f = () => { return 1; }; return f(); }
  get cond() { if (Math.random()) { return 1; } }
  private get p() { return 1; }
  public set p(v: number) {}
  protected get q() { return 1; }
  private set q(v: number) {}
  protected get r() { return 1; }
  set r(v: number) {}
  abstract get ab(): number;
  set ab(v: number) {}
  static get s() { return 1; }
  private static set s(v: number) {}
}
export const o = { get x() { }, get y() { return 1; } };
declare class D { get z(): number; }
