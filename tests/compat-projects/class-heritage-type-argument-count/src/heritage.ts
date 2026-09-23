class C9 extends Array { }
class C10 extends Array<number> { }

interface Mup<K, V> { readonly size: number }
interface MupConstructor {
    new(): Mup<any, any>;
    new<K, V>(entries?: readonly (readonly [K, V])[] | null): Mup<K, V>;
    readonly prototype: Mup<any, any>;
}
declare var Mup: MupConstructor;
class Sizz extends Mup { }

interface I<T> { foo: T }
class D extends I { }

class G<T> { g!: T }
class FromClass extends G { }
class FromClassOk extends G<number> { }
class FromClassTooMany extends G<number, string> { }

class Defaulted<T, U = string> { t!: T; u!: U }
class FromDefaulted extends Defaulted { }
class FromDefaultedOk extends Defaulted<number> { }

class Box<T> { value!: T }
class ArgumentPosition extends Box<G> { }
