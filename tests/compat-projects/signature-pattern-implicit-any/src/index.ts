interface Methods {
    tuple([, a, b]): void;
    object({ p, m: { q, r } }): void;
    renamed({ key: local }): void;
    defaults([a = 1, { b } = { b: 2 }]): void;
    rests([...only], { ...others }): void;
    arrayRest([first, ...more]): void;
    optional([x, y]?): void;
}

type Call = {
    ([a, b]): void;
    new ({ c }): object;
};

type Fn = ([d]) => void;
type Ctor = new ({ e }) => object;

export type { Methods, Call, Fn, Ctor };
