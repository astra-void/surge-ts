interface Labelled {
    new (label: string): { render(): void };
}
declare const Primitive: Labelled;

interface Either {
    new (label: string): { render(): void };
    new (count: number): { render(): void };
}
declare const Overloaded: Either;

// With no `ElementAttributesProperty` the props are the first constructor
// parameter; no attributes object is assignable to a primitive.
export const single = <Primitive x={10} />;
export const bare = <Primitive />;
export const overloaded = <Overloaded x={10} />;
