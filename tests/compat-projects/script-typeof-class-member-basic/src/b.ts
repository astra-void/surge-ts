var local: { bar: number };

class Holder {
    fromOther!: typeof shared;
    fromThisFile!: typeof local;
    static fromStatic: typeof local;
    method(value: typeof shared) { return value.foo; }
    set value(next: typeof local) { }
    callback!: (arg: typeof shared) => void;
}

const wrong: string = new Holder().fromThisFile.bar;
