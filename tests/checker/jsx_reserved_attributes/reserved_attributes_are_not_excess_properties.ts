// @jsx: preserve
// @filename: example.tsx
// `key` is accepted because `JSX.IntrinsicAttributes` declares it, not because
// tsc reserves the name: without that interface it is an excess property.
declare global { namespace JSX { interface Element {} interface IntrinsicElements { div: {} } interface IntrinsicAttributes { key?: string | number } } }
interface Props { task: string }
declare function Item(props: Props): JSX.Element;
export const a = <Item key="1" task="x" />;
