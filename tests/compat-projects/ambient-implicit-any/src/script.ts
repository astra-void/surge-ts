declare var untyped;
declare let first, second: number, third;
declare const typed: string;
declare namespace Ambient.Nested {
    export var inner;
    let hidden;
}
declare global {
    var fromGlobal;
}
declare class Members {
    shown;
    private hidden;
    #secret;
    static shared;
}
export {};
