const [a, b] = [10, "hello"];
const aNumber: number = a;
const bString: string = b;
const aWrong: string = a;
const { 0: c, 1: d } = [10, "hello"];
const cNumber: number = c;
const dWrong: number = d;
const { 1: e } = [10, "hello"];
const eUnion: string | number = e;
const eWrong: string = e;
const [x, [y, [z]]] = [1, ["hello", [true]]];
const yString: string = y;
const zWrong: string = z;
const { k: [m, n] } = { k: [1, "s"] };
const nWrong: number = n;
const [p, q, r] = [1, "x"];
const [u] = [];
const [h, ...rest] = [1, "x", true];
const restTuple: [string, boolean] = rest;
function scoped() {
    const { 0: first, 1: second } = [10, "hello"];
    const secondWrong: number = second;
    return first;
}
