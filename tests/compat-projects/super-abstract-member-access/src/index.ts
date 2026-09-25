class A {
    foo() {
        return 1;
    }
}

abstract class B extends A {
    abstract foo(): number;
    bar() {
        return super.foo();
    }
}

export class C extends B {
    foo() {
        return 2;
    }
    qux() {
        return super.foo() || super.foo;
    }
    norf() {
        return super.bar();
    }
}
