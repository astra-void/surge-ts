interface Labelled {
    new (label: string): { render(): void };
}
declare const Primitive: Labelled;

interface Titled {
    new (props: { title: string }): { render(): void };
}
declare const Framed: Titled;

// Props of `IntrinsicAttributes & IntrinsicClassAttributes<…> & string` are no
// excess-property target, so the relation names the constituent that fails:
// the missing `key`, then `string` itself.
const primitive = <Primitive x={10} />;
const keyed = <Primitive key="k" x={10} />;

// An object props type still makes the intersection one.
const extra = <Framed key="k" title="t" x={10} />;
