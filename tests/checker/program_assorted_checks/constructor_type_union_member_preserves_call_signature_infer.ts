// A constructor signature (`new (...) => T`) as a union member must not collapse
// the whole union to `unknown`. This mirrors React's
// `JSXElementConstructor<P> = ((props: P) => …) | (new (props: P) => …)`; the
// props type is recovered from the call-signature member.
type ElementCtor<P> = ((props: P) => string) | (new (props: P) => object);
type PropsOf<T> = T extends ElementCtor<infer P> ? P : never;
declare const widget: (props: { title: string }) => string;
type R = PropsOf<typeof widget>;
const bad: R = { other: 1 };
