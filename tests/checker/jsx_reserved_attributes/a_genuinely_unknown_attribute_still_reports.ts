// @jsx: preserve
// @filename: example.tsx
declare global { namespace JSX { interface Element {} interface IntrinsicElements { div: {} } interface IntrinsicAttributes { key?: string | number } } }
interface Props { task: string }
declare function Item(props: Props): JSX.Element;
export const a = <Item nope="1" task="x" />;
