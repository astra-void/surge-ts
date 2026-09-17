export {}
class Base { m(x: number): string { return "" + x } p: number = 1; opt?: string }
interface Shape { area(): number; name: string; tag?: string }

class GoodDerived extends Base { m(x: number): string { return "y" } p: number = 2 }
class NarrowProp extends Base { p: 5 = 5 }
class AddOptional extends Base { opt?: string = "a" }
class ExtraMember extends Base { extra: boolean = true }
class GoodImpl implements Shape { area() { return 1 } name: string = "g" }
class ExtraImpl implements Shape { area() { return 1 } name: string = "e"; more: number = 2 }

class BadReturn extends Base { m(x: number): number { return x } }
class BadProp extends Base { p: string = "s" }
class BadOptional extends Base { opt: number = 1 }
class BadParam extends Base { m(x: string): string { return x } }
class BadArea implements Shape { area(): string { return "" } name: string = "b" }
class BadName implements Shape { area() { return 1 } name: number = 1 }
class MissingAll implements Shape {}
class BadAndMissing implements Shape { area(): string { return "" } }
