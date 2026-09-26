declare const Count: number;
declare const Heading: "h1";
declare const Division: "div";
declare function Returns(): string;
declare function Renders(): JSX.Element;
declare const Shape: { width: number };

// Nothing to call: TS2604 at the tag.
export const counted = <Count />;
export const shaped = <Shape />;
// A string-literal tag is the intrinsic element it names, when there is one.
export const heading = <Heading />;
export const division = <Division title="t" />;
export const divisionWrong = <Division title={1} />;

// What a component returns or constructs must be a JSX element: TS2786.
export const returnsString = <Returns />;
export const renders = <Renders />;

class NoRender {
    props!: { label: string };
}
class NoProps {
    render() { return null; }
}
class Complete {
    props!: { label: string };
    render() { return null; }
}
export const noRender = <NoRender label="l" />;
// An instance without the `ElementAttributesProperty` member takes no
// attributes: TS2607 at the element, only when it is given some.
export const noProps = <NoProps label="l" />;
export const noPropsBare = <NoProps />;
export const complete = <Complete label="l" />;
