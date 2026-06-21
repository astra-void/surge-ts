export function run(set: Set<number>, map: Map<string, number>, list: number[]): void {
  for (const value of set) {
    const wrong: string = value;
    void wrong;
  }

  for (const [key, count] of map) {
    const k: string = key;
    const c: number = count;
    void k;
    void c;
  }

  for (const item of list.values()) {
    const n: number = item;
    void n;
  }

  for (const [index, item] of list.entries()) {
    const i: number = index;
    const v: number = item;
    void i;
    void v;
  }
}
