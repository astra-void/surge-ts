class Base {
  static origin: string = 'base';
  static describe(): string {
    return Base.origin;
  }
  instanceOfBase(): void {}
}

class Middle extends Base {
  static level: number = 2;
  instanceOfMiddle(): void {}
}

class Leaf extends Middle {
  static override describe(): string {
    return 'leaf';
  }
  instanceOfLeaf(): void {}
}

export const inheritedProperty: string = Leaf.origin;
export const inheritedThroughTheChain: number = Leaf.level;
export const overriddenMethod: string = Leaf.describe();

export function prototypeStaysTheDerivedInstance(): void {
  Leaf.prototype.instanceOfLeaf();
  Leaf.prototype.instanceOfBase();
}

declare class AmbientBase {
  static tag: number;
}

declare class AmbientDerived extends AmbientBase {}

export const ambientStatic: number = AmbientDerived.tag;

export const wrongType: number = Leaf.origin;
export const neverDeclared = Leaf.missing;
