interface Shape {
    name: string;
    width: number;
    visible: boolean;
}

type Dictionary<T> = { [key: string]: T };
type NumericallyIndexed<T> = { [key: number]: T };

enum Direction { Up, Down }

// A string index signature answers numeric keys as well.
type D1 = Dictionary<string>[number];
type D2 = Dictionary<string>[Direction.Down];
type D3 = Dictionary<string>[`${number}`];

// A number index signature answers numeric string keys and `${number}`.
type N1 = NumericallyIndexed<boolean>["12"];
type N2 = NumericallyIndexed<boolean>[`${number}`];

// A tuple names its elements; past them it reads its rest elements.
type Pair = [string, number];
type P1 = Pair[Direction.Up];
type P2 = Pair["1"];
type Rest = [number, string, ...boolean[]];
type R1 = Rest[2 | 3];

// A union receiver reads the key from every member.
type Either = [boolean] | [string, number];
type E1 = Either[0];
type E2 = Either[1];
type E3 = Either[number];

// Arrays and primitives are read through their apparent type.
type A1 = boolean[]["length"];
type A2 = boolean[][0];
type S1 = string[number];

const d1: D1 = "text";
const d2: D2 = "text";
const d3: D3 = "text";
const n1: N1 = true;
const n2: N2 = false;
const p1: P1 = "text";
const p2: P2 = 1;
const r1: R1 = true;
const e1: E1 = false;
const e2: E2 = undefined;
const e3: E3 = 1;
const a1: A1 = 3;
const a2: A2 = true;
const s1: S1 = "c";

class Store<Props> {
    set<K extends keyof Props>(key: K, value: Props[K]): void {}
}

// The error follows the key's kind: a literal names a missing property, `string`
// and `number` find no index signature, and anything else is no key at all.
type Bad1 = Shape[string];
type Bad2 = Shape[number];
type Bad3 = Shape[boolean];
type Bad4 = Shape["size"];
type Bad5 = Shape[any];
type Bad6 = Shape[string | boolean];
type Bad7 = boolean[][string];
type Bad8 = Either[2];
type Bad9 = Pair[2];
type Bad10 = NumericallyIndexed<boolean>[string];

const badD1: D1 = 1;
const badE2: E2 = "text";
const badR1: R1 = undefined;
