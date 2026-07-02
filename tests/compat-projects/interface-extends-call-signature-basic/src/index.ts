interface Base<P> {
  (props: P): string;
}
interface Named<P> extends Base<P> {
  displayName?: string;
}
interface Exotic<P> extends Named<P> {
  readonly brand?: symbol;
}

declare const Comp: Exotic<{ a: string }>;

type PropsOf<T> = T extends (props: infer P) => unknown ? P : never;

const bad: PropsOf<typeof Comp> = 5;

export const ok: PropsOf<typeof Comp> = { a: "x" };
export { bad };
