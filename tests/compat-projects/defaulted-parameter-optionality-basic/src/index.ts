interface State { count: number }
const initial: State = { count: 0 };

function reducer(state = initial, action: { type: string }): State {
    const inside: State = state;
    return action.type === "inc" ? { count: inside.count + 1 } : state;
}
reducer(undefined, { type: "inc" });
reducer(null, { type: "inc" });

function annotated(a: string | undefined = "x", b: number) {
    const s: string = a;
    return s + b;
}
annotated(undefined, 1);
annotated(1, 1);

const arrow = (a: number = 1, b: string) => a.toFixed() + b;
arrow(undefined, "x");

class K {
    constructor(a = 1, b: string) {}
    m(a: string = "d", b: number) { return a.length + b; }
}
new K(undefined, "s").m(undefined, 2);
new K("no", "s");

function trailing(a: number, b = "z") { return a + b.length; }
trailing(1, undefined);
trailing(1);

const wide: (a: State | undefined, action: { type: string }) => State = reducer;
type P = Parameters<typeof reducer>;
const p: P = [undefined, { type: "x" }];
const q: P = [1, { type: "x" }];

function declaredWithUndefined(x: string | undefined = "s", b: number) {
    x.length;
    x = undefined;
    return b;
}
function undefinedDefault(x: string | undefined = undefined, b: number) {
    x.length;
    return b;
}
export {};
