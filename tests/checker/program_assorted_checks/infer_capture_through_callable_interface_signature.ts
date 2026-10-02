// `infer` capture against a callable object (an interface carrying a call
// signature, e.g. React's `ForwardRefExoticComponent<P>`) resolves through the
// object's call signature rather than degrading.
interface Callable { (props: { id: string }): void; }
declare const callable: Callable;
type FirstParam<T> = T extends (props: infer P) => any ? P : never;
type R = FirstParam<typeof callable>;
const bad: R = { nope: 1 };
