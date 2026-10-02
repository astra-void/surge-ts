// @noImplicitAny: true
// @filename: src/index.tsx
// A JSX component whose props type could not be modelled offers no contextual
// type for an inline callback attribute, so reporting implicit-any there would
// describe surge's own gap rather than the source — the radix
// `ComponentProps<typeof Primitive.Root>` cluster. A component with a real
// props type still reports.
declare function Widget(props: { label: string }): null;
export const a = <Widget onPick={(value) => value} />;
