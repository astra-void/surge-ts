// The result stays unmodelled: `void 0` must not start reporting as `undefined`,
// and a well-formed operand reports nothing.
type Plain = { value: string; opt?: number };
declare const g: Plain;
declare const n: number;
const a = void 0;
const b = ~n;
delete g.opt;
void g.value;
export { a, b };
