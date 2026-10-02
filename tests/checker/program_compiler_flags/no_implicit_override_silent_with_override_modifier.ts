// @noImplicitOverride: true
class Base { greet(): string { return 'a'; } } class Derived extends Base { override greet(): string { return 'b'; } }
