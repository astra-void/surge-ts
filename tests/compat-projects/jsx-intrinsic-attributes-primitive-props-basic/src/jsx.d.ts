declare namespace JSX {
    interface Element { __element: true }
    interface ElementClass { render(): unknown }
    interface IntrinsicAttributes { key: string | number }
    interface IntrinsicClassAttributes<T> { ref?: T }
    interface IntrinsicElements { [name: string]: unknown }
}
