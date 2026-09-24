interface Render { (value: number): string }
declare function One(props: { children: Render }): JSX.Element;
declare function Many(props: { children: Render[] }): JSX.Element;
declare function Optional(props: { children?: Render; id?: string }): JSX.Element;

const render: Render = (value) => `${value}`;

// Without `ElementChildrenAttribute` the body is no attribute, so `children`
// is missing; a failed relation is still checked for arity under `children`.
const one = <One>{render}</One>;
const tooMany = <One>{render}{render}</One>;
const tooFew = <Many>{render}</Many>;
const many = <Many>{render}{render}</Many>;

// A relation that holds is never elaborated.
const optional = <Optional>{render}{render}</Optional>;
const optionalMismatch = <Optional id={1}>{render}{render}</Optional>;
