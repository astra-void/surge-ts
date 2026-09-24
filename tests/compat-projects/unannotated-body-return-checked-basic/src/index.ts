export function pick(flag: boolean) {
  if (flag) {
    return { kind: 'a' as const, value: 1 };
  }
  const helper = () => 'x';
  return { kind: 'b' as const, value: helper() };
}
const y1: 0 = pick(true);

export function withLocal(n: number) {
  type Local = { n: number };
  const local: Local = { n };
  return local;
}
const y2: 0 = withLocal(1);

export function maybe(n: number) {
  if (n > 0) {
    return n;
  }
}
const y3: 0 = maybe(1);

let side = 0;
export function noReturn() {
  side++;
}
const y4: 0 = noReturn();

export function viaNested(items: number[]) {
  const doubled = items.map((item) => item * 2);
  return { doubled, total: doubled.reduce((sum, item) => sum + item, 0) };
}
const y5: 0 = viaNested([1]);
