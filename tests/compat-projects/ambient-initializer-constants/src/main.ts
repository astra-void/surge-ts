export class Fields {
    declare x: number;
    declare y = 1;
    static declare z = 2;
    declare readonly w = 3;
}

declare class Ambient {
    a = 1;
    readonly b = 2;
    readonly c = "s" + "t";
}

declare enum Flag { On = 1 }

declare namespace Values {
    const literal = 1;
    let mutable = 2;
    const typed: number = 3;
    const template = `t`;
    const negative = -1;
    const sum = 1 + 1;
    const flag = true;
    const big = 10n;
    const member = Flag.On;
    const element = Flag["On"];
}

export type Kept = [typeof Ambient, typeof Values];
