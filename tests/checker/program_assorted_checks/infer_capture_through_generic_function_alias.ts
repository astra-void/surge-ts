// `infer` capture reached by expanding a generic alias whose body is a function
// (`Ctor<infer P>`), matching React's `JSXElementConstructor<infer Props>` shape.
type Ctor<P> = (props: P) => unknown;
type PropsOf<T> = T extends Ctor<infer P> ? P : never;
declare const comp: (props: { id: string }) => unknown;
type R = PropsOf<typeof comp>;
const bad: R = { nope: 1 };
