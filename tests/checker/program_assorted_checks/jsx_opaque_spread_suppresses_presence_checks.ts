// @filename: example.tsx
// An opaque spread (`any`, unresolved) folds the whole attributes object into
// `any` in tsc, so both the missing-required and excess checks stand down.
declare function Item(props: { label: string }): null;
declare const rest: any;
const ok = <Item {...rest} bonus={1} />;
