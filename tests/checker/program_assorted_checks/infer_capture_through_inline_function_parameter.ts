// `infer` capture inside a function-parameter position resolves to the matched
// argument: the excess-property error proves `R` is the concrete `{ id: string }`
// props object, not a degraded `unknown`/`any`. Regression guard for the
// function-pattern arm of conditional `infer` binding (the core of
// `React.ComponentProps<typeof FunctionComponent>`).
type FirstParam<T> = T extends (props: infer P) => any ? P : never;
declare const comp: (props: { id: string }) => void;
type R = FirstParam<typeof comp>;
const bad: R = { nope: 1 };
