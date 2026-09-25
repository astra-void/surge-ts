class Early extends Late {}
class Late {}

class A extends C {}
class B extends A {}
class C extends B {}

interface Base extends Derived2 {
    x: string;
}
interface Derived extends Base {
    y: string;
}
interface Derived2 extends Derived {
    z: string;
}

namespace Generic {
    interface Base<T> extends Derived<T> {}
    interface Derived<T> extends Base<T> {}
}

interface Left {
    shared: string;
    left: number;
}
interface Right {
    shared: number;
    right: number;
}
interface Both extends Left, Right {}

interface Box<T> {
    value: T;
}
interface Pair<T> extends Box<number>, Box<string> {}

interface Holder<T> {
    item: T;
    make: () => T;
}
interface NarrowHolder<T> extends Holder<T> {
    item: T;
}
interface WrongHolder<T> extends Holder<T> {
    item: { wrapped: T };
}
interface Base2 {
    a: <T>() => T;
}
interface Takes<T> extends Base2 {
    a: () => T;
}
