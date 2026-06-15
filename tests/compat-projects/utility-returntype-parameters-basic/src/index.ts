type Fn = (id: string, count: number) => { id: string; count: number };

type R = ReturnType<Fn>;
type P = Parameters<Fn>;

const r: R = { id: "a", count: 1 };
const rBad: R = { id: "a", count: "x" };

const p: P = ["a", 1];
const pBad: P = ["a", "x"];
