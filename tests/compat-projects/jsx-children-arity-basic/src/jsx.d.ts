declare namespace JSX {
    interface Element { __element: true }
    interface ElementChildrenAttribute { children: {} }
    interface IntrinsicElements { [name: string]: unknown }
}
