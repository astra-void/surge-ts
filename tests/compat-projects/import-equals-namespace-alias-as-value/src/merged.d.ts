declare module "merged" {
    namespace B {
        export interface A { a: number }
    }
    interface B {
        bar(name: string): B.A;
    }
    export = B;
}
