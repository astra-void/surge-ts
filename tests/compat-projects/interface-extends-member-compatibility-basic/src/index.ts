export {}
interface Base { m(x: number): string; p: number; opt?: string }
interface A { x: number }
interface B { y: string }

interface GoodChild extends Base { extra: boolean }
interface NarrowProp extends Base { p: 5 }
interface BothOk extends A, B { z: boolean }

interface BadReturn extends Base { m(x: number): number }
interface BadProp extends Base { p: string }
interface BadOpt extends Base { opt: number }
interface Multi extends Base { m(x: number): number; p: string }
interface BadOfTwo extends A, B { x: string }
