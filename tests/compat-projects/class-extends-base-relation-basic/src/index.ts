export {};
class A { x = 1; static s = 1; protected p = 1; private q = 1; m() {} }
class B1 extends A { private x = 2 }
class B2 extends A { protected x = 2 }
class B3 extends A { q = 2 }
class B4 extends A { private q = 2 }
class B5 extends A { static s = "a" }
class B6 extends A { p = 2 }
class B7 extends A { x: string = "a" }
class B8 extends A { private m() {} }
class B9 extends A { readonly x = 3 }
class B10 extends A { static s = 2 }
class B11 extends A { private static s = 2 }
abstract class B12 extends A { abstract x: number }
