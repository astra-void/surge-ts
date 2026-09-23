namespace Shapes {
    export interface Point { x: number }
    declare const p: Point;
    p.x; p.y; p.label; p.fromOtherFile;
    p.z;
}

namespace Shapes {
    export interface Point { y: number }
    export interface Point { label: string }
    declare const p: Point;
    p.x; p.y; p.label; p.fromOtherFile;
    p.z;
}

namespace Shapes.Nested {
    export interface Box { width: number }
}

namespace Shapes {
    export namespace Nested {
        export interface Box { height: number }
        declare const b: Box;
        b.width; b.height;
        b.depth;
    }
}

declare namespace Ambient {
    interface Config { a: string }
}

declare namespace Ambient {
    interface Config { b: string }
}

namespace Local {
    interface Hidden { a: string }
    interface Hidden { b: string }
    declare const h: Hidden;
    h.a; h.b;
    h.c;
}

namespace WithClass {
    export class Widget { size = 1 }
}

namespace WithClass {
    export interface Widget { color: string }
    declare const w: Widget;
    w.size; w.color;
    w.weight;
}

declare const point: Shapes.Point;
point.x; point.y; point.label; point.fromOtherFile;
declare const box: Shapes.Nested.Box;
box.width; box.height;
declare const config: Ambient.Config;
config.a; config.b;
