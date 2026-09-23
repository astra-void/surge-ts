type Circle = { kind: "circle"; radius: number };
type Square = { kind: "square"; size: number };
declare function Shape(props: Circle | Square): JSX.Element;
declare function Card(props: { title: string; subtitle: string; body?: string }): JSX.Element;
declare function Badge(props: { label: string }): JSX.Element;

class Panel {
    props!: { heading: string };
    render() { return null; }
}

// With both intrinsic attribute interfaces declared, tsc names the
// constituent that misses the props, not the whole intersection.
const missingOne = <Badge />;
const missingTwo = <Card />;
const missingClass = <Panel />;
const keyed = <Badge key="k" label="l" />;
const classRef = <Panel ref={(panel) => panel.render()} heading="h" />;

// A union is narrowed by the discriminant the attributes write before the
// excess check.
const circle = <Shape kind="circle" radius={1} />;
const squareWithRadius = <Shape kind="square" radius={1} />;
const squareWithSize = <Shape kind="square" size={1} />;
