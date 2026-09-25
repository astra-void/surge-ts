class Base {
    foo(v: string) {}
    fooo(v: string) {}
    x = 1;
    static make() { return new Base(); }
}

class Derived extends Base {
    override foo(v: string) {}
    fooo(v: string) {}
    override bar() {}
    override fooz() {}
    toString() { return ""; }
    static make() { return new Derived(1, ""); }
    static bind() {}
    constructor(public x: number, override y: string) { super(); }
}

class Alone {
    override m() {}
}

abstract class AbstractBase {
    abstract run(): void;
    walk() {}
}

abstract class AbstractDerived extends AbstractBase {
    abstract run(): void;
    walk() {}
}

class FromLib extends Error {
    message = "x";
    override missing = 1;
}

class FromArray extends Array<number> {
    override lengthh = 1;
}

let key = "k";
const sym: symbol = Symbol();

class Dynamic extends Base {
    override [key]() {}
    [sym]() {}
}

class Params {
    constructor(readonly public a: number, override public b: number) {}
}

abstract class Order {
    override abstract f(): void;
    accessor abstract g: number;
}

type Variance<out in T> = T;

class Ctors {
    static constructor() {}
}

declare function decorate(target: unknown): void;
@decorate
default class {}
