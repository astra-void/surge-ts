declare namespace Outer {
    declare var value: number;
    export declare function run(): void;
    declare namespace Inner { }
    declare class Box { }
    function body(): void {}
    function* generate(): Iterable<number>;
    module Legacy { }
}
declare module Dotted.Path {
    export declare const flag: boolean;
}
declare module "ambient-module" {
    declare const setting: string;
    export default 1 + 1;
}
declare class Holder {
    *items(): Iterable<string>;
}
