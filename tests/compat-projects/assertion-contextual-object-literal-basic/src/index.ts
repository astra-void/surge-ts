interface Ctx {
  handle(input: number): number;
  tag: string;
}

export const method = { handle(x) { return x; }, tag: 't' } as Ctx;
export const arrow = { handle: (x) => x, tag: 't' } as Ctx;
export const angle = <Ctx>{ handle(x) { return x; }, tag: 't' };
export const omitted = {} as Ctx;
export const extra = { handle(x) { return x; }, tag: 't', more: 1 } as Ctx;

export const inner = {
  handle(x) {
    const label: string = x;
    return label.length;
  },
  tag: 't',
} as Ctx;
