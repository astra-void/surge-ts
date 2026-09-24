interface Render { (value: number): string }
declare function One(props: { children: Render }): JSX.Element;
declare function Many(props: { children: Render[] }): JSX.Element;
declare function Either(props: { children: Render | Render[] }): JSX.Element;
declare function Label(props: { children?: Render }): JSX.Element;

const render: Render = (value) => `${value}`;

// Several children against a type with no iterable member is TS2746 at the
// tag; one child against an iterable-only type is TS2745 there.
const tooMany = <One>{render}{render}</One>;
const tooFew = <Many>{render}</Many>;
const tooFewText = <Many>text</Many>;
const eitherOne = <Either>{render}</Either>;
const eitherMany = <Either>{render}{render}</Either>;

// Text does not fit a function: TS2747 at the text.
const text = <One>text</One>;
const optionalText = <Label>
    text
</Label>;
