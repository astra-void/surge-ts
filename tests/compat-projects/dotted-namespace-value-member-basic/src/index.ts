namespace Outer.Middle {
    export function greet() { return "hi"; }
    export const count = 1;
}
namespace Outer.Middle.Inner {
    export const flag = true;
}
namespace Outer {
    export const top = 2;
}
declare namespace Ambient.Sub {
    function measure(): number;
}

const greeting: string = Outer.Middle.greet();
const total: number = Outer.Middle.count + Outer.top;
const flagged: boolean = Outer.Middle.Inner.flag;
const measured: number = Ambient.Sub.measure();
const absent = Outer.Middle.missing;
