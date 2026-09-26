declare namespace JSX {
    interface Element { __element: true }
    interface ElementClass { render(): unknown }
    interface ElementAttributesProperty { props: {} }
    interface IntrinsicElements { div: { title?: string } }
}
