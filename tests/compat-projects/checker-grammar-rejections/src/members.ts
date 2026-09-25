class Accessors {
    get value(): number;
    set value(next: number);
}

interface Overloads {
    run(a: string): void;
    run?(a: number): void;
}

type Literal = {
    go(): void;
    go?(x: number): void;
};

var indexKey: { [key: string, other: string]: number };
var missingAnnotation: { [key: string] };
var invalidKey: { [key: any] };
export {};
