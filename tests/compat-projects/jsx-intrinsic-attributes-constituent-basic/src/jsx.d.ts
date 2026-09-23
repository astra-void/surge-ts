declare namespace JSX {
    interface Element { __element: true }
    interface ElementAttributesProperty { props: {} }
    interface IntrinsicAttributes { key?: string }
    interface IntrinsicClassAttributes<T> { ref?: (instance: T) => void }
    interface IntrinsicElements { [name: string]: unknown }
}
