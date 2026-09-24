declare namespace JSX {
    interface Element { __element: true }
    interface ElementAttributesProperty { props: {} }
    interface ElementChildrenAttribute { children: {} }
    interface IntrinsicAttributes { key?: string }
    interface IntrinsicElements {
        box: { width: number; height?: number };
        pair: { first: string; second: string; third?: string };
        data: { [name: string]: string };
    }
}
