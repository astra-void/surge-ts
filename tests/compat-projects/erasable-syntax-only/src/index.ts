enum Color {
    Red,
}

declare enum Ambient {
    A,
}

const enum Folded {
    B,
}

namespace Instantiated {
    export const value = 1;
}

namespace TypesOnly {
    export type T = string;
}

declare namespace AmbientNamespace {
    const inner: number;
}

import alias = Instantiated.value;

class Point {
    constructor(public x: number, readonly y: number, private z: number) {}
}

const asserted = <string>(alias as unknown);

export { Color, Folded, Point, asserted };
export type { TypesOnly, Ambient, AmbientNamespace };
