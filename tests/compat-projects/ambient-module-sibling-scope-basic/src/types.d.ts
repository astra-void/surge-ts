declare module "libmod" {
    interface Outer {
        inner(): Inner;
    }
    interface Inner {
        deep(): Deep;
    }
    interface Deep {
        value: number;
    }
    type Level = "a" | "b";
    global {
        namespace FixtureNS {
            interface Carrier {
                pick(level: Level): Deep;
            }
        }
    }
    function make(): Outer;
    export { Outer, Inner, Deep, make };
}
