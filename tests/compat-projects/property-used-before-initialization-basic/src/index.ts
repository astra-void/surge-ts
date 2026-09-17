export {}
class DeclaredEarlier {
  a = 1;
  b = this.a;
}
class DeferredByArrow {
  a = () => this.b;
  b = 1;
}
class DeferredInsideObject {
  a = { get: () => this.b };
  b = 1;
}
class ReadsMethod {
  a = this.m();
  m() { return 1 }
}
class OptionalTarget {
  a = this.b;
  b?: number;
}
class AssertedTarget {
  a = this.b;
  b!: number;
}

class ReadsLaterProperty {
  a = this.b;
  b = 1;
}
class ReadsLaterStatic {
  static s1 = ReadsLaterStatic.s2;
  static s2 = 1;
}
class ReadsUninitializedProperty {
  a: number;
  b = this.a;
  constructor() { this.a = 1 }
}
