declare namespace JSX {
    interface Element { __element: true }
    interface IntrinsicElements {
        box: { x: string; y?: number };
    }
}
