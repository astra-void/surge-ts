export namespace dom {
    namespace JSX {
        interface IntrinsicElements {
            p: { x?: number };
        }
        interface Element {
            __domBrand: void;
        }
    }
}
export function dom(): dom.JSX.Element;
