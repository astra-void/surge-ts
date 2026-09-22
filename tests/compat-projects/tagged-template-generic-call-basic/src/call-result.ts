interface I {
    (s: string): I;
    [x: number]: I;
    m: number;
}
declare var f: I;
const x = f("");
const y: never = x[0];
const z: never = f("")[0];
const w: never = f("").m;
declare function g(s: string): I;
const v: never = g("")[0];
