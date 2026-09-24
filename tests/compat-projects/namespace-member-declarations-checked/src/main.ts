namespace N {
    export interface Exported { p: Missing1; ok: Local; }
    interface Local { p: Missing2; self: Exported; }
    export type Alias = Missing3;
    type LocalAlias = { p: Missing4; ok: Alias };
    export class C { p: Missing5; ok: Local; }
    export namespace M { export interface Nested { p: Missing6; ok: Exported; } }
    export function f(x: Missing7) {}
}
namespace P.Q { export interface Dotted { p: Missing8; } }
declare namespace D { interface Ambient { p: Missing9; m?(x: Missing10): boolean; } }
