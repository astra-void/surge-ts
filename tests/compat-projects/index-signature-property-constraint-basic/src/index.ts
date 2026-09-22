export {};

// Every property must fit each index signature that applies to its name.
interface Local {
    [key: string]: string | number;
    ok: string;
    alsoOk: 1;
    bad: boolean;
    method(): void;
}

interface Numeric {
    [index: number]: string;
    0: string;
    1: number;
    named: boolean;
}

// The number index must fit the string one.
interface BothIndexes {
    [key: string]: string;
    [index: number]: number;
}

// Inherited index, own property: reported on the property.
interface Base {
    [key: string]: { x: number };
}
interface Derived extends Base {
    a: { x: number; y: string };
    b: { y: number };
}

// Own index, inherited property: reported on the index signature.
interface HasProperty {
    p: boolean;
}
interface AddsIndex extends HasProperty {
    [key: string]: number;
}

// Both inherited from different bases: reported on the interface name.
interface BaseWithIndex {
    [key: string]: number;
}
interface Both extends HasProperty, BaseWithIndex {}

// A base that already holds both reports it; the derived interface does not.
interface Holder {
    [key: string]: number;
    q: string;
}
interface Inherits extends Holder {}

export class Cls {
    [key: string]: number | (() => void);
    count = 1;
    label = "x";
    run() {}
}

// A generic declaration is checked over its own type variables.
interface Generic<T> {
    [key: string]: string;
    value: T;
    fixed: string;
}
