export const o = {
  _v: 1,
  get v(): number { return this._v; },
  set v(next: number) { const w: boolean = next; this._v = next; },
  set only(x: string) { const n: number = x; },
};
interface Box { v: number }
export const typed: Box = {
  get v(): number { return 1; },
  set v(next) { const s: string = next; },
};
