class Registry {
    static readonly [key: string]: number;
    static readonly [key: number]: 42;
}
Registry["label"] = 1;
Registry.other = 2;
Registry[42] = 42;
delete Registry.gone;
Registry.count++;

interface Frozen { readonly [key: string]: number }
declare const frozen: Frozen;
frozen.a = 1;
frozen["b"] = 2;

interface Base { readonly [key: string]: number }
interface Derived extends Base { own: number }
declare const derived: Derived;
derived.own = 1;
derived.inherited = 2;

declare const view: ArrayLike<number>;
view[0] = 1;

enum Color { Red, Green }
Color[0] = "Blue";

declare const either: string | number[];
either[0] = 1;
if (typeof either !== "string") {
    either[0] = 1;
}

declare const writable: { [key: string]: number };
writable.a = 1;
writable["b"] = 2;
