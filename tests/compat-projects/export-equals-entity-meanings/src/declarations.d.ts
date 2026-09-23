declare namespace foo.bar {
    export type X = number;
    export const X: number;
    export interface Shape { size: number }
}

declare module "foobar" {
    export = foo.bar;
}

declare module "foobarx" {
    export = foo.bar.X;
}
