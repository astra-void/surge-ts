declare function random(): number;
declare function mix<T>(base: T): T;
let key: string = "k";
export class Base {}
export class Members {
    field = random();
    readonly fixed = 1;
    private hidden = random();
    #secret = random();
    method() {}
    typed(): void {}
    get getter() { return random(); }
    set setter(value) {}
    [key] = 1;
    [key + "m"]() {}
    constructor(public p = random()) {}
}
export class Mixed extends mix(Base) {}
export const expression = class {};
export interface Shape {
    sized;
    measure();
}
export enum Local {
    A = 1,
    B = A << 1,
    C = random(),
}
const external = 3;
export enum Reads {
    D = external,
    E = D + 1,
    F = Local.A,
}
