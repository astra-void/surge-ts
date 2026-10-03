// @target: es2015
class Person {
    ["B"](a: number): string;
    ["A"](a: string | number): number | string {
        return 0;
    }
}
class C {
    ["foo"](): void
    ["bar"](): void;
    ["foo"]() {
        return 0;
    }
}
class D {
    [Symbol.iterator](x: any): any;
    [Symbol.iterator](x: any) { return undefined; }
    [Symbol.iterator](x: any) { return undefined; }
}
