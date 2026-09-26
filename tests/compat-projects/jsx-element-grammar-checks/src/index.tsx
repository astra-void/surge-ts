declare var React: any;
declare global {
    namespace JSX {
        interface IntrinsicElements { [name: string]: any }
    }
}

// A namespaced tag is intrinsic only when its namespace is: TS2639 otherwise.
export const svg = <svg:path />;
export const custom = <Custom:path />;

// An attribute assigned an empty expression is TS17000, the element's first
// grammar error only.
export const empty = <div onClick={} title={} />;
export const repeated = <div title="a" title="b" onClick={} />;
