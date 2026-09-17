declare function tag(...args: unknown[]): any;

class Base {
  run() {}
  static make() {}
  value = 1;
}

class Ordered extends Base {
  override static make() {}
  static public create() {}
  readonly public label = "a";
  readonly static shared = 1;
  override public run() {}
  async public load() {}
  async static fetch() {}
  override readonly value = 2;
  @tag static public decorated() {}
  static /* note */ public commented() {}

  public static readonly fine = 1;
  private static async fineAsync() {}
  protected override readonly other = 3;
}

abstract class Shape {
  public abstract area(): number;
  abstract public perimeter(): number;
}
