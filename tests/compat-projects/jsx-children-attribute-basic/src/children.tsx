declare function Text(props: { children: string }): JSX.Element;
declare function MaybeText(props: { children?: string }): JSX.Element;
declare function Numbers(props: { children: number[] }): JSX.Element;
declare function Count(props: { children: number }): JSX.Element;
declare function Leaf(props: { id: string }): JSX.Element;

// A string children type is iterable, so one child that does not fit is
// TS2745 at the tag; with `undefined` in the type it is reported at the child.
const text = <Text>{1}</Text>;
const maybeText = <MaybeText>{1}</MaybeText>;
const numbers = <Numbers>{"x"}</Numbers>;
const count = <Count>{"x"}</Count>;
const element = <Count><Leaf id="i" /></Count>;
const layout = (
    <Text>
        hello
    </Text>
);

// The body is only excess-checked when an attribute is written.
const bodyOnly = <Leaf>x</Leaf>;
const bodyWithAttribute = <Leaf id="i">x</Leaf>;
