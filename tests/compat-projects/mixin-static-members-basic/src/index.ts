class A {
    constructor(...args: any[]) {}
    static sa: string = "";
    static fa(): number { return 1; }
    shared = 1;
}

class B {
    constructor(...args: any[]) {}
    static sb: number = 0;
    shared = 2;
}

declare function Mix<T, U>(c1: T, c2: U): T & U;
declare function Id<T>(c: T): T;

class C extends Mix(A, B) {
    static own = true;
    static g() {
        C.sa; C.sb; C.fa(); C.own;
        C.missing;
    }
}

C.sa.length; C.sb.toFixed(); C.fa();
C.missing;
const sa: number = C.sa;

class D extends Id(A) {}
D.sa; D.fa();
D.sb;

class E extends Mix(C, B) {}
E.sa; E.sb; E.own;
E.none;
