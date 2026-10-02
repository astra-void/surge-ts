// @noImplicitOverride: true
class A { f(): number { return 1; } } class B extends A {} class C extends B { f(): number { return 2; } }
