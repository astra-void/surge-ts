export namespace Outer {
    export interface Point {
        x: number;
    }
    export import Nested = Inner;
    namespace Inner {
        export const value = 1;
    }
}

export function misplaced() {
    namespace Local { }
    export namespace Exported { }
    namespace Dotted.Name { }
    declare module "ambient" { }
    declare global { }
    import Alias = Outer;
    import * as Everything from "./other";
    import "./other";
    export * from "./other";
    export { misplaced };
    export default 1;
    export default class Klass { }
    export function inner() { }
    export = Outer;
}

label: namespace Labeled { }
