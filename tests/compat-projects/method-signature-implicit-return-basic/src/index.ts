interface Applicable {
  apply(blah: any);
}

class Holder {
  public prop() {}
}
class StaticHolder {
  public static prop() {}
}
interface NeedsProp {
  prop();
}

export function members(target: Applicable, needs: NeedsProp): void {
  target = "";
  target = 4;
  target = {};
  target = function () {};
  const result: void = target.apply(1);
  needs = new StaticHolder();
  needs = Holder;
  needs = new Holder();
}
