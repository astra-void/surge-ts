export class Base {
  prop = 1;
  get acc() { return 1; }
  method() { return 1; }
  private priv = 1;
  static sprop = 1;
  set onlySet(v: number) {}
}
export class D1 extends Base { get prop() { return 2; } }
export class D2 extends Base { acc = 3; }
export class D3 extends Base { method = () => 1; }
export class D4 extends Base { get method() { return () => 1; } }
export class D5 extends Base { prop() { return 1; } }
export class D7 extends Base { static get sprop() { return 1; } }
export class D8 extends D1 { prop = 5; }
export class D9 extends Base { declare acc: number; }
export class D10 extends Base { onlySet = 1; }
export abstract class AB { abstract a: number; abstract get b(): number; }
export class D11 extends AB { get a() { return 1; } b = 2; }
